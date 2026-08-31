"""Python parity contract for deterministic context dependency closure."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

CONTEXT_DEPENDENCY_CLOSURE_FEATURE_ID = "AFA-brain-P03-F10"
CONTEXT_DEPENDENCY_CLOSURE_CONTRACT_VERSION = "brain-context-dependency-closure/1.0"


@dataclass(frozen=True)
class BrainContextDependencyClosureReceipt:
    request_id: str
    objective: str
    disposition: str
    context_order: tuple[str, ...]
    resolved_order: tuple[str, ...]
    missing_dependency_order: tuple[str, ...]
    cycle_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    dependency_order: tuple[str, ...]
    closure_digest: str
    context_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = CONTEXT_DEPENDENCY_CLOSURE_FEATURE_ID
    contract_version: str = CONTEXT_DEPENDENCY_CLOSURE_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != CONTEXT_DEPENDENCY_CLOSURE_FEATURE_ID or self.contract_version != CONTEXT_DEPENDENCY_CLOSURE_CONTRACT_VERSION:
            raise ResearchContractError("dependency closure schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.objective.strip() or not self.context_order or not self.effect_receipts or self.disposition not in {"qualified", "partial", "unknown", "blocked"}:
            raise ResearchContractError("dependency closure identity, graph, locality, disposition, or effects are incomplete")
        for values in (self.context_order, self.resolved_order, self.missing_dependency_order, self.cycle_order, self.blocked_order, self.dependency_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("dependency closure vectors are not canonical")
        contexts = set(self.context_order); classified = set(self.resolved_order) | set(self.missing_dependency_order) | set(self.cycle_order) | set(self.blocked_order)
        if classified != contexts:
            raise ResearchContractError("dependency closure context states do not partition contexts")
        for value in (self.closure_digest, self.context_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("dependency closure digest is invalid")
        if any(not effect.startswith("compile:local-dependency-closure:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("dependency closure effect is outside local compilation gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "objective": self.objective, "disposition": self.disposition, "context_order": list(self.context_order), "resolved_order": list(self.resolved_order), "missing_dependency_order": list(self.missing_dependency_order), "cycle_order": list(self.cycle_order), "blocked_order": list(self.blocked_order), "dependency_order": list(self.dependency_order), "closure_digest": self.closure_digest, "context_digest": self.context_digest, "replay_identity": self.replay_identity, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})


def compile_context_dependency_closure(*, request_id: str, objective: str, required_context_ids: Sequence[str], available_context_ids: Sequence[str], edges: Sequence[Mapping[str, Any]], replay_identity: str, policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True) -> BrainContextDependencyClosureReceipt:
    if not request_id.strip() or not objective.strip() or not required_context_ids or not re.fullmatch(r"[0-9a-f]{64}", replay_identity):
        raise ResearchContractError("dependency closure identity, contexts, or replay is invalid")
    contexts = tuple(sorted(set(required_context_ids))); available = set(available_context_ids)
    if len(contexts) != len(required_context_ids) or any(not value.strip() for value in contexts):
        raise ResearchContractError("context identifiers must be unique and non-empty")
    outgoing: dict[str, set[str]] = {value: set() for value in contexts}; indegree = {value: 0 for value in contexts}; missing = set(contexts) - available
    for edge in edges:
        context_id = str(edge["context_id"]); dependency_id = str(edge["dependency_id"])
        if context_id not in contexts:
            continue
        if dependency_id not in available or dependency_id not in contexts:
            missing.add(context_id); continue
        if context_id not in outgoing[dependency_id]:
            outgoing[dependency_id].add(context_id); indegree[context_id] += 1
    ready = sorted(value for value, degree in indegree.items() if degree == 0); topo: list[str] = []
    while ready:
        value = ready.pop(0); topo.append(value)
        for child in sorted(outgoing[value]):
            indegree[child] -= 1
            if indegree[child] == 0:
                ready.append(child); ready.sort()
    cycle = {value for value, degree in indegree.items() if degree > 0}; resolved = {value for value in topo if value not in missing and value not in cycle}; blocked: set[str] = set()
    if not policy_allow or not protected_closure or not raw_data_local:
        blocked = set(contexts); resolved.clear()
    missing -= blocked; cycle -= blocked; cycle -= missing
    omissions = {f"context:{value}:missing-dependency" for value in missing} | {f"context:{value}:dependency-cycle" for value in cycle}
    if blocked: omissions.add("context:policy-protected-closure-locality-blocked")
    uncertainty = {"context:dependency-replay-mismatch"} if any(str(edge.get("replay_identity")) != replay_identity for edge in edges) else set(); negative: set[str] = set()
    disposition = "blocked" if blocked else ("unknown" if not resolved else ("qualified" if len(resolved) == len(contexts) and not missing and not cycle and not uncertainty else "partial"))
    closure_digest = research_artifact_digest({"context_order": list(contexts), "dependency_order": topo, "missing": sorted(missing), "cycle": sorted(cycle), "replay_identity": replay_identity}); context_digest = research_artifact_digest({"feature_id": CONTEXT_DEPENDENCY_CLOSURE_FEATURE_ID, "request_id": request_id, "closure_digest": closure_digest}); effects = (f"compile:local-dependency-closure:{request_id}",) if disposition in {"qualified", "partial"} else ("block:unsafe-release",); artifact = {"content_hash": research_artifact_digest({"request_id": request_id, "context_digest": context_digest}), "media_type": "application/vnd.aurora.context-dependency-closure+json"}
    receipt = BrainContextDependencyClosureReceipt(request_id=request_id, objective=objective, disposition=disposition, context_order=contexts, resolved_order=tuple(sorted(resolved)), missing_dependency_order=tuple(sorted(missing)), cycle_order=tuple(sorted(cycle)), blocked_order=tuple(sorted(blocked)), dependency_order=tuple(topo), closure_digest=closure_digest, context_digest=context_digest, replay_identity=replay_identity, omissions=tuple(sorted(omissions)), uncertainty=tuple(sorted(uncertainty)), negative_evidence=tuple(sorted(negative)), effect_receipts=effects, artifact=artifact)
    receipt.validate(); return receipt
