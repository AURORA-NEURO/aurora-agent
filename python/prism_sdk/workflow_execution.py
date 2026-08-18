"""Typed workflow-bound simulation and replay for Agent Interweave workflows.

The facade mirrors the Rust binding rather than pretending the workflow catalogue is an external
orchestration engine.  A successful response is still a receipt projection, and a missing grant
is represented as a structured refusal inside that receipt.
"""

from __future__ import annotations

import json
import math
import re
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .adaptive_execution import _array, _digest, _finite
from .capability import _route_mapping, _route_text
from .errors import ArgumentError

WORKFLOW_EXECUTION_SCHEMA = "bioprism-interweave/workflow-execution/0.1"
INTERWEAVE_WORKFLOW_IDS = (
    "reliable_software_repair",
    "scientific_claim_reproduction",
    "biomedical_research_data_audit",
    "incident_response",
    "evidence_grounded_policy_comparison",
    "dataset_transformation_molecule",
)
_DIGEST = re.compile(r"^[0-9a-f]{64}$")


def _workflow_payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = dict(value)
    if raw.get("schema") == WORKFLOW_EXECUTION_SCHEMA:
        return raw
    mcp = raw.get("mcp")
    if isinstance(mcp, Mapping):
        result = mcp.get("result")
        if isinstance(result, Mapping):
            structured = result.get("structuredContent")
            if isinstance(structured, Mapping):
                return _workflow_payload(structured)
            content = result.get("content")
            if isinstance(content, Sequence) and not isinstance(content, (str, bytes)):
                for block in content:
                    if isinstance(block, Mapping) and isinstance(block.get("text"), str):
                        decoded = json.loads(block["text"])
                        if isinstance(decoded, Mapping):
                            return _workflow_payload(decoded)
    raise ArgumentError("response does not contain a workflow execution projection")


def _objects(name: str, value: Any, *, maximum: int) -> tuple[Mapping[str, Any], ...]:
    rows = _array(name, value)
    if len(rows) > maximum:
        raise ArgumentError(f"{name} must contain at most {maximum} rows")
    if any(not isinstance(row, Mapping) for row in rows):
        raise ArgumentError(f"each {name} row must be an object")
    return tuple(rows)


@dataclass(frozen=True)
class WorkflowExecutionRequest:
    """A validated request for one closed reference workflow identity."""

    workflow: str
    problem: Mapping[str, Any]
    belief: Mapping[str, Any]
    acquisitions: Sequence[Mapping[str, Any]]
    budget: float
    max_steps: int
    mode: str = "simulate"
    provider: str = "mcp-simulated"
    capabilities: Sequence[str] = ()
    authorization: Mapping[str, Any] | None = None
    observations: Sequence[Mapping[str, Any]] = ()
    receipt: Mapping[str, Any] | None = None
    evidence: Mapping[str, Any] | None = None

    def __post_init__(self) -> None:
        if self.workflow not in INTERWEAVE_WORKFLOW_IDS:
            raise ArgumentError("workflow must be one of the six reference workflow ids")
        if not isinstance(self.problem, Mapping) or not self.problem:
            raise ArgumentError("problem must be a non-empty mapping")
        if not isinstance(self.belief, Mapping) or not self.belief:
            raise ArgumentError("belief must be a non-empty mapping")
        if not isinstance(self.acquisitions, Sequence) or isinstance(self.acquisitions, (str, bytes)) or not 1 <= len(self.acquisitions) <= 16:
            raise ArgumentError("acquisitions must contain 1..=16 rows")
        if any(not isinstance(row, Mapping) for row in self.acquisitions):
            raise ArgumentError("each acquisition must be an object")
        _finite("budget", self.budget)
        if self.budget < 0.0:
            raise ArgumentError("budget must be non-negative")
        if not isinstance(self.max_steps, int) or isinstance(self.max_steps, bool) or not 0 <= self.max_steps <= 16:
            raise ArgumentError("max_steps must be 0..=16")
        if self.mode not in {"simulate", "replay"}:
            raise ArgumentError("mode must be simulate or replay")
        if not isinstance(self.provider, str) or not self.provider.strip() or len(self.provider) > 256:
            raise ArgumentError("provider must be a visible string of at most 256 characters")
        if not isinstance(self.capabilities, Sequence) or isinstance(self.capabilities, (str, bytes)) or len(self.capabilities) > 32:
            raise ArgumentError("capabilities must contain at most 32 labels")
        if any(not isinstance(label, str) or not label.strip() for label in self.capabilities):
            raise ArgumentError("capabilities must contain non-empty strings")
        if not isinstance(self.observations, Sequence) or isinstance(self.observations, (str, bytes)) or len(self.observations) > 16:
            raise ArgumentError("observations must contain at most 16 rows")
        if any(not isinstance(row, Mapping) for row in self.observations):
            raise ArgumentError("each observation must be an object")
        if self.authorization is not None and not isinstance(self.authorization, Mapping):
            raise ArgumentError("authorization must be an object")
        if self.receipt is not None and not isinstance(self.receipt, Mapping):
            raise ArgumentError("receipt must be an object")
        if self.evidence is not None:
            if not isinstance(self.evidence, Mapping):
                raise ArgumentError("evidence must be an object")
            subject_id = self.evidence.get("subject_id")
            domains = self.evidence.get("domains")
            if not isinstance(subject_id, str) or not subject_id.strip():
                raise ArgumentError("evidence.subject_id must be a non-empty string")
            if not isinstance(domains, Sequence) or isinstance(domains, (str, bytes)) or not 1 <= len(domains) <= 64:
                raise ArgumentError("evidence.domains must contain 1..=64 labels")
            if any(not isinstance(domain, str) or not domain.strip() for domain in domains):
                raise ArgumentError("evidence.domains must contain non-empty strings")
            parents = self.evidence.get("parent_digests", [])
            if not isinstance(parents, Sequence) or isinstance(parents, (str, bytes)) or len(parents) > 128:
                raise ArgumentError("evidence.parent_digests must contain at most 128 digests")
            for digest in parents:
                if not isinstance(digest, str) or not _DIGEST.fullmatch(digest):
                    raise ArgumentError("evidence.parent_digests must contain lowercase SHA-256 digests")
        if self.mode == "replay" and self.receipt is None:
            raise ArgumentError("receipt is required in replay mode")

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "WorkflowExecutionRequest":
        raw = _route_mapping("workflow execution request", value)
        acquisitions = _objects("workflow execution acquisitions", raw.get("acquisitions"), maximum=16)
        observations = _objects("workflow execution observations", raw.get("observations", []), maximum=16)
        capabilities = raw.get("capabilities", [])
        if not isinstance(capabilities, Sequence) or isinstance(capabilities, (str, bytes)):
            raise ArgumentError("workflow execution capabilities must be an array")
        return cls(
            workflow=_route_text("workflow execution workflow", raw.get("workflow")),
            problem=_route_mapping("workflow execution problem", raw.get("problem")),
            belief=_route_mapping("workflow execution belief", raw.get("belief")),
            acquisitions=acquisitions,
            budget=_finite("workflow execution budget", raw.get("budget")),
            max_steps=raw.get("max_steps"),
            mode=raw.get("mode", "simulate"),
            provider=raw.get("provider", "mcp-simulated"),
            capabilities=tuple(capabilities),
            authorization=dict(raw["authorization"]) if isinstance(raw.get("authorization"), Mapping) else raw.get("authorization"),
            observations=observations,
            receipt=dict(raw["receipt"]) if isinstance(raw.get("receipt"), Mapping) else raw.get("receipt"),
            evidence=dict(raw["evidence"]) if isinstance(raw.get("evidence"), Mapping) else raw.get("evidence"),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "workflow": self.workflow,
            "problem": dict(self.problem),
            "belief": dict(self.belief),
            "acquisitions": [dict(row) for row in self.acquisitions],
            "budget": self.budget,
            "max_steps": self.max_steps,
            "mode": self.mode,
            "provider": self.provider,
            "capabilities": list(self.capabilities),
            "observations": [dict(row) for row in self.observations],
        }
        if self.authorization is not None:
            result["authorization"] = dict(self.authorization)
        if self.receipt is not None:
            result["receipt"] = dict(self.receipt)
        if self.evidence is not None:
            result["evidence"] = dict(self.evidence)
        return result


@dataclass(frozen=True)
class WorkflowExecutionReport:
    raw: dict[str, Any]
    schema: str
    workflow: str
    mode: str
    plan_digest: str
    binding_digest: str
    completed: bool
    release_posture: str
    status: str
    refusal: str | None
    provenance_counts: Mapping[str, int]
    binding: Mapping[str, Any]
    receipt: Mapping[str, Any]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "WorkflowExecutionReport":
        raw = _workflow_payload(value)
        if raw.get("ok") is not True:
            raise ArgumentError("workflow execution response is not successful")
        schema = _route_text("workflow execution schema", raw.get("schema"))
        if schema != WORKFLOW_EXECUTION_SCHEMA:
            raise ArgumentError("workflow execution schema is invalid")
        workflow = _route_text("workflow execution workflow", raw.get("workflow"))
        if workflow not in INTERWEAVE_WORKFLOW_IDS:
            raise ArgumentError("workflow execution workflow is invalid")
        mode = _route_text("workflow execution mode", raw.get("mode"))
        if mode not in {"simulate", "replay"}:
            raise ArgumentError("workflow execution mode is invalid")
        plan_digest = _digest("workflow execution plan_digest", raw.get("plan_digest"))
        binding_digest = _digest("workflow execution binding_digest", raw.get("binding_digest"))
        binding = _route_mapping("workflow execution binding", raw.get("binding"))
        if binding.get("workflow") != workflow or binding.get("binding_digest") != binding_digest:
            raise ArgumentError("workflow execution binding does not reconcile")
        completed = raw.get("completed")
        if not isinstance(completed, bool):
            raise ArgumentError("workflow execution completed must be boolean")
        release_posture = _route_text("workflow execution release posture", raw.get("release_posture"))
        receipt = _route_mapping("workflow execution receipt", raw.get("receipt"))
        if receipt.get("schema") != WORKFLOW_EXECUTION_SCHEMA or receipt.get("workflow") != workflow or receipt.get("binding_digest") != binding_digest:
            raise ArgumentError("workflow execution receipt does not reconcile")
        adaptive = _route_mapping("workflow execution adaptive receipt", receipt.get("adaptive"))
        if adaptive.get("plan_digest") != plan_digest:
            raise ArgumentError("workflow execution adaptive plan digest does not reconcile")
        status = _route_text("workflow execution status", adaptive.get("status"))
        if status not in {"completed", "partial", "refused"}:
            raise ArgumentError("workflow execution status is invalid")
        if completed != (status == "completed"):
            raise ArgumentError("workflow execution completed flag does not reconcile")
        refusal = adaptive.get("refusal")
        if refusal is not None:
            refusal = _route_text("workflow execution refusal", refusal)
        rows = _array("workflow execution observations", adaptive.get("observations"))
        counts = _route_mapping("workflow execution provenance_counts", raw.get("provenance_counts"))
        normalized: dict[str, int] = {}
        for name in ("observed", "simulated", "replayed"):
            count = counts.get(name)
            if not isinstance(count, int) or isinstance(count, bool) or count < 0:
                raise ArgumentError(f"workflow execution {name} count must be a non-negative integer")
            actual = 0
            for row in rows:
                observation = _route_mapping("workflow execution observation row", row).get("observation")
                observation = _route_mapping("workflow execution observation", observation)
                if observation.get("provenance") == name:
                    actual += 1
            if count != actual:
                raise ArgumentError("workflow execution provenance counts do not reconcile")
            normalized[name] = count
        return cls(raw, schema, workflow, mode, plan_digest, binding_digest, completed, release_posture, status, refusal, normalized, dict(binding), dict(receipt))

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def workflow_execution_report(value: Mapping[str, Any]) -> WorkflowExecutionReport:
    """Parse a direct MCP result or HTTP tool envelope."""

    return WorkflowExecutionReport.from_wire(value)


__all__ = [
    "INTERWEAVE_WORKFLOW_IDS",
    "WORKFLOW_EXECUTION_SCHEMA",
    "WorkflowExecutionRequest",
    "WorkflowExecutionReport",
    "workflow_execution_report",
]
