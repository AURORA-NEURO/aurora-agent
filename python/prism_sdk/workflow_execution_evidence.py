"""Typed portable evidence for workflow-bound execution receipts.

The evidence facade keeps the receipt's workflow identity and provenance visible while exposing
the bounded registry operations.  It does not interpret a receipt as provider authentication,
scientific truth, clinical evidence, or release authority.
"""

from __future__ import annotations

import json
import re
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .adaptive_execution import _array
from .authoring import content_digest
from .capability import _route_mapping, _route_text
from .errors import ArgumentError

WORKFLOW_EXECUTION_EVIDENCE_SCHEMA = "bioprism-devplat-workflow-execution-evidence/0.1"
WORKFLOW_EXECUTION_EVIDENCE_WORKFLOW = "interweave_workflow_execution_evidence"
WORKFLOW_EXECUTION_EVIDENCE_IMPORT_SCHEMA = "bioprism-devplat-workflow-execution-evidence-import/0.1"
WORKFLOW_EXECUTION_EVIDENCE_QUERY_SCHEMA = "bioprism-devplat-workflow-execution-evidence-query/0.1"
WORKFLOW_EXECUTION_EVIDENCE_GET_SCHEMA = "bioprism-devplat-workflow-execution-evidence-get/0.1"
_DIGEST = re.compile(r"^[0-9a-f]{64}$")


def _evidence_payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = dict(value)
    if raw.get("schema") in {
        WORKFLOW_EXECUTION_EVIDENCE_SCHEMA,
        WORKFLOW_EXECUTION_EVIDENCE_IMPORT_SCHEMA,
        WORKFLOW_EXECUTION_EVIDENCE_QUERY_SCHEMA,
        WORKFLOW_EXECUTION_EVIDENCE_GET_SCHEMA,
    }:
        return raw
    mcp = raw.get("mcp")
    if isinstance(mcp, Mapping):
        result = mcp.get("result")
        if isinstance(result, Mapping):
            structured = result.get("structuredContent")
            if isinstance(structured, Mapping):
                return _evidence_payload(structured)
            content = result.get("content")
            if isinstance(content, Sequence) and not isinstance(content, (str, bytes)):
                for block in content:
                    if isinstance(block, Mapping) and isinstance(block.get("text"), str):
                        decoded = json.loads(block["text"])
                        if isinstance(decoded, Mapping):
                            return _evidence_payload(decoded)
    raise ArgumentError("response does not contain workflow execution evidence")


def _digest(name: str, value: Any) -> str:
    text = _route_text(name, value)
    if not _DIGEST.fullmatch(text):
        raise ArgumentError(f"{name} must be a lowercase 64-character SHA-256 digest")
    return text


def _strings(name: str, value: Any, *, maximum: int, required: bool = True) -> tuple[str, ...]:
    if value is None and not required:
        return ()
    rows = _array(name, value)
    if required and not rows:
        raise ArgumentError(f"{name} must contain at least one label")
    if len(rows) > maximum:
        raise ArgumentError(f"{name} must contain at most {maximum} labels")
    if any(not isinstance(row, str) or not row.strip() for row in rows):
        raise ArgumentError(f"{name} must contain non-empty strings")
    return tuple(rows)


@dataclass(frozen=True)
class WorkflowExecutionEvidenceRequest:
    """Validated input for receipt-to-evidence conversion and registry import."""

    binding: Mapping[str, Any]
    receipt: Mapping[str, Any]
    subject_id: str
    domains: Sequence[str]
    parent_digests: Sequence[str] = ()

    def __post_init__(self) -> None:
        if not isinstance(self.binding, Mapping) or not self.binding:
            raise ArgumentError("binding must be a non-empty mapping")
        if not isinstance(self.receipt, Mapping) or not self.receipt:
            raise ArgumentError("receipt must be a non-empty mapping")
        if not isinstance(self.subject_id, str) or not self.subject_id.strip() or len(self.subject_id) > 512:
            raise ArgumentError("subject_id must be a visible string of at most 512 bytes")
        _strings("workflow execution evidence domains", self.domains, maximum=64)
        parents = _strings("workflow execution evidence parent_digests", self.parent_digests, maximum=128, required=False)
        for digest in parents:
            _digest("workflow execution evidence parent digest", digest)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "WorkflowExecutionEvidenceRequest":
        raw = _route_mapping("workflow execution evidence request", value)
        return cls(
            binding=_route_mapping("workflow execution evidence binding", raw.get("binding")),
            receipt=_route_mapping("workflow execution evidence receipt", raw.get("receipt")),
            subject_id=_route_text("workflow execution evidence subject_id", raw.get("subject_id")),
            domains=_strings("workflow execution evidence domains", raw.get("domains"), maximum=64),
            parent_digests=_strings("workflow execution evidence parent_digests", raw.get("parent_digests", []), maximum=128, required=False),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "binding": dict(self.binding),
            "receipt": dict(self.receipt),
            "subject_id": self.subject_id,
            "domains": list(self.domains),
        }
        if self.parent_digests:
            result["parent_digests"] = list(self.parent_digests)
        return result


@dataclass(frozen=True)
class WorkflowExecutionEvidenceReport:
    raw: dict[str, Any]
    schema: str
    workflow: str
    evidence_digest: str
    evidence: Mapping[str, Any] | None
    registry: Mapping[str, Any] | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "WorkflowExecutionEvidenceReport":
        raw = _evidence_payload(value)
        if raw.get("ok") is not True:
            raise ArgumentError("workflow execution evidence response is not successful")
        schema = _route_text("workflow execution evidence schema", raw.get("schema"))
        if schema not in {
            WORKFLOW_EXECUTION_EVIDENCE_SCHEMA,
            WORKFLOW_EXECUTION_EVIDENCE_IMPORT_SCHEMA,
            WORKFLOW_EXECUTION_EVIDENCE_GET_SCHEMA,
        }:
            raise ArgumentError("workflow execution evidence schema is invalid")
        workflow = _route_text("workflow execution evidence workflow", raw.get("workflow"))
        digest = _digest("workflow execution evidence digest", raw.get("evidence_digest"))
        evidence = raw.get("evidence")
        if evidence is not None:
            evidence = _route_mapping("workflow execution evidence record", evidence)
            if evidence.get("schema") != WORKFLOW_EXECUTION_EVIDENCE_SCHEMA:
                raise ArgumentError("workflow execution evidence record schema is invalid")
            if evidence.get("evidence_digest") != digest:
                raise ArgumentError("workflow execution evidence digest does not reconcile")
            unsigned = dict(evidence)
            unsigned.pop("evidence_digest", None)
            if content_digest(unsigned) != digest:
                raise ArgumentError("workflow execution evidence digest is not canonical")
            if evidence.get("readiness_claimed") is not False or evidence.get("execution") != "not_started":
                raise ArgumentError("workflow execution evidence readiness posture is invalid")
        registry = raw.get("registry")
        if registry is not None:
            registry = _route_mapping("workflow execution evidence registry", registry)
        return cls(raw, schema, workflow, digest, evidence, registry)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def workflow_execution_evidence_report(value: Mapping[str, Any]) -> WorkflowExecutionEvidenceReport:
    """Parse a direct MCP result or HTTP tool envelope."""

    return WorkflowExecutionEvidenceReport.from_wire(value)


__all__ = [
    "WORKFLOW_EXECUTION_EVIDENCE_GET_SCHEMA",
    "WORKFLOW_EXECUTION_EVIDENCE_IMPORT_SCHEMA",
    "WORKFLOW_EXECUTION_EVIDENCE_QUERY_SCHEMA",
    "WORKFLOW_EXECUTION_EVIDENCE_SCHEMA",
    "WORKFLOW_EXECUTION_EVIDENCE_WORKFLOW",
    "WorkflowExecutionEvidenceReport",
    "WorkflowExecutionEvidenceRequest",
    "workflow_execution_evidence_report",
]
