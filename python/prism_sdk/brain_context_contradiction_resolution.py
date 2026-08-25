"""Python parity contract for contradiction-resolution planning."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

CONTEXT_CONTRADICTION_RESOLUTION_FEATURE_ID = "AFA-brain-P03-F09"
CONTEXT_CONTRADICTION_RESOLUTION_CONTRACT_VERSION = "brain-context-contradiction-resolution/1.0"


@dataclass(frozen=True)
class BrainContextContradictionResolutionReceipt:
    request_id: str
    objective: str
    disposition: str
    group_order: tuple[str, ...]
    resolved_group_order: tuple[str, ...]
    contested_group_order: tuple[str, ...]
    missing_group_order: tuple[str, ...]
    blocked_group_order: tuple[str, ...]
    unknown_group_order: tuple[str, ...]
    resolution_plan_order: tuple[str, ...]
    conflict_digest: str
    context_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = CONTEXT_CONTRADICTION_RESOLUTION_FEATURE_ID
    contract_version: str = CONTEXT_CONTRADICTION_RESOLUTION_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != CONTEXT_CONTRADICTION_RESOLUTION_FEATURE_ID or self.contract_version != CONTEXT_CONTRADICTION_RESOLUTION_CONTRACT_VERSION:
            raise ResearchContractError("contradiction-resolution schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.objective.strip() or not self.group_order or not self.resolution_plan_order or not self.effect_receipts or self.disposition not in {"qualified", "partial", "unknown", "blocked"}:
            raise ResearchContractError("contradiction-resolution identity, groups, plan, locality, disposition, or effects are incomplete")
        for values in (self.group_order, self.resolved_group_order, self.contested_group_order, self.missing_group_order, self.blocked_group_order, self.unknown_group_order, self.resolution_plan_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("contradiction-resolution vectors are not canonical")
        groups = set(self.group_order); classified = set(self.resolved_group_order) | set(self.contested_group_order) | set(self.missing_group_order) | set(self.blocked_group_order) | set(self.unknown_group_order)
        if classified != groups:
            raise ResearchContractError("contradiction-resolution group states do not partition groups")
        for value in (self.conflict_digest, self.context_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("contradiction-resolution digest is invalid")
        if any(not effect.startswith("compile:local-contradiction-resolution:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("contradiction-resolution effect is outside local compilation gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "objective": self.objective, "disposition": self.disposition, "group_order": list(self.group_order), "resolved_group_order": list(self.resolved_group_order), "contested_group_order": list(self.contested_group_order), "missing_group_order": list(self.missing_group_order), "blocked_group_order": list(self.blocked_group_order), "unknown_group_order": list(self.unknown_group_order), "resolution_plan_order": list(self.resolution_plan_order), "conflict_digest": self.conflict_digest, "context_digest": self.context_digest, "replay_identity": self.replay_identity, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})


def compile_context_contradiction_resolution(*, request_id: str, objective: str, required_group_ids: Sequence[str], claims: Sequence[Mapping[str, Any]], minimum_support_milli: int, replay_identity: str, policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True) -> BrainContextContradictionResolutionReceipt:
    if not request_id.strip() or not objective.strip() or not required_group_ids or not re.fullmatch(r"[0-9a-f]{64}", replay_identity):
        raise ResearchContractError("contradiction-resolution identity, groups, or replay is invalid")
    groups = tuple(sorted(set(required_group_ids))); by_group: dict[str, list[Mapping[str, Any]]] = {}
    for claim in claims:
        by_group.setdefault(str(claim["conflict_group"]), []).append(claim)
    if len(groups) != len(required_group_ids) or any(not value.strip() for value in groups):
        raise ResearchContractError("group identifiers must be unique and non-empty")
    resolved: list[str] = []; contested: list[str] = []; missing: list[str] = []; blocked: list[str] = []; unknown: list[str] = []; plans: list[str] = []; omissions: list[str] = []; uncertainty: list[str] = []; negative: list[str] = []
    for group in groups:
        items = by_group.get(group, [])
        if not items:
            missing.append(group); omissions.append(f"group:{group}:missing-claims"); continue
        if not policy_allow or not protected_closure or not raw_data_local or any(not bool(item.get("provenance_complete", False)) or not bool(item.get("raw_data_local", True)) or str(item.get("boundary", PRECLINICAL_BOUNDARY)) != PRECLINICAL_BOUNDARY for item in items):
            blocked.append(group); omissions.append(f"group:{group}:policy-provenance-locality-blocked"); continue
        if any(str(item.get("replay_identity")) != replay_identity for item in items):
            unknown.append(group); uncertainty.append(f"group:{group}:replay-mismatch"); continue
        supported = [item for item in items if str(item.get("state")) == "supported" and int(item.get("support_milli", 0)) >= minimum_support_milli]; polarities = {str(item.get("polarity", "")) for item in supported}
        if len(polarities) > 1:
            contested.append(group); plans.append(f"group:{group}:retain-competing-and-replicate"); negative.append(f"group:{group}:contradictory-supported-claims")
        elif len(supported) == 1:
            resolved.append(group); plans.append(f"group:{group}:retain-supported-claim")
        elif any(str(item.get("state")) in {"unknown", "speculative"} for item in items):
            unknown.append(group); plans.append(f"group:{group}:acquire-discriminating-evidence"); uncertainty.append(f"group:{group}:unresolved-claim-state")
        else:
            blocked.append(group); plans.append(f"group:{group}:below-support-or-unproven"); omissions.append(f"group:{group}:no-supported-claim")
    if not plans: plans.append("plan:none")
    disposition = "blocked" if not policy_allow or not protected_closure or not raw_data_local else ("unknown" if not resolved else ("qualified" if len(resolved) == len(groups) and not contested and not missing and not blocked and not unknown else "partial"))
    conflict_digest = research_artifact_digest({"group_order": list(groups), "resolved": resolved, "contested": contested, "missing": missing, "blocked": blocked, "unknown": unknown, "plan": sorted(plans), "replay_identity": replay_identity}); context_digest = research_artifact_digest({"feature_id": CONTEXT_CONTRADICTION_RESOLUTION_FEATURE_ID, "request_id": request_id, "conflict_digest": conflict_digest, "negative": negative}); effects = (f"compile:local-contradiction-resolution:{request_id}",) if disposition in {"qualified", "partial"} else ("block:unsafe-release",); artifact = {"content_hash": research_artifact_digest({"request_id": request_id, "context_digest": context_digest}), "media_type": "application/vnd.aurora.context-contradiction-resolution+json"}
    receipt = BrainContextContradictionResolutionReceipt(request_id=request_id, objective=objective, disposition=disposition, group_order=groups, resolved_group_order=tuple(sorted(resolved)), contested_group_order=tuple(sorted(contested)), missing_group_order=tuple(sorted(missing)), blocked_group_order=tuple(sorted(blocked)), unknown_group_order=tuple(sorted(unknown)), resolution_plan_order=tuple(sorted(plans)), conflict_digest=conflict_digest, context_digest=context_digest, replay_identity=replay_identity, omissions=tuple(sorted(omissions)), uncertainty=tuple(sorted(uncertainty)), negative_evidence=tuple(sorted(negative)), effect_receipts=effects, artifact=artifact)
    receipt.validate(); return receipt
