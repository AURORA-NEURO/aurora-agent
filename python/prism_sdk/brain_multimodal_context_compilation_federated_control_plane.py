"""Multimodal multi-study context control-plane parity contract."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    MULTIMODAL_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_CONTRACT_VERSION,
    MULTIMODAL_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class MultimodalContextControlCell:
    study_id: str
    modality: str
    context_digest: str
    section_digest: str
    evidence_digest: str | None
    provenance_digest: str | None
    replay_identity: str
    state: str = "supported"
    comparable: bool = True
    ready: bool = True
    retry_count: int = 0
    telemetry_digest: str | None = None
    cost_units: int = 1
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY


@dataclass(frozen=True)
class MultimodalContextControlReceipt:
    request_id: str
    workspace_id: str
    workflow_id: str
    scope: str
    goal: str
    disposition: str
    study_order: tuple[str, ...]
    modality_order: tuple[str, ...]
    cell_order: tuple[str, ...]
    completed_order: tuple[str, ...]
    degraded_order: tuple[str, ...]
    unresolved_order: tuple[str, ...]
    denied_order: tuple[str, ...]
    incomparable_order: tuple[str, ...]
    exchange_order: tuple[str, ...]
    checkpoint_seq: int
    retry_count: int
    consumed_budget_units: int
    run_digest: str
    telemetry_digest: str
    federation_digest: str
    replay_identity: str
    witness_order: tuple[str, ...]
    counterexample_order: tuple[str, ...]
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = MULTIMODAL_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_FEATURE_ID
    contract_version: str = MULTIMODAL_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != MULTIMODAL_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_FEATURE_ID or self.contract_version != MULTIMODAL_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_CONTRACT_VERSION:
            raise ResearchContractError("multimodal control schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.workspace_id.strip() or not self.workflow_id.strip() or not self.scope.strip() or not self.goal.strip() or len(self.study_order) < 2 or len(self.modality_order) < 2 or not self.cell_order or self.checkpoint_seq != len(self.cell_order) or not self.effect_receipts or self.disposition not in {"completed", "degraded", "unresolved", "denied"}:
            raise ResearchContractError("multimodal control identity, matrix closure, checkpoint, locality, or effects are incomplete")
        for values in (self.study_order, self.modality_order, self.cell_order, self.completed_order, self.degraded_order, self.unresolved_order, self.denied_order, self.incomparable_order, self.exchange_order, self.witness_order, self.counterexample_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("multimodal control ordering is not canonical")
        classified = set(self.completed_order) | set(self.degraded_order) | set(self.unresolved_order) | set(self.denied_order)
        if classified != set(self.cell_order):
            raise ResearchContractError("multimodal control dispositions do not partition cells")
        if len(self.exchange_order) != len(self.completed_order):
            raise ResearchContractError("multimodal control exchange does not match completed cells")
        for value in (*self.exchange_order, self.run_digest, self.telemetry_digest, self.federation_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("multimodal control digest is invalid")
        if any(not effect.startswith("exchange:permitted-multimodal-summary:") and not effect.startswith("manage:multimodal-context:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("multimodal control effect is outside the governed operations gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "workspace_id": self.workspace_id, "workflow_id": self.workflow_id, "scope": self.scope, "goal": self.goal, "disposition": self.disposition, "study_order": list(self.study_order), "modality_order": list(self.modality_order), "cell_order": list(self.cell_order), "completed_order": list(self.completed_order), "degraded_order": list(self.degraded_order), "unresolved_order": list(self.unresolved_order), "denied_order": list(self.denied_order), "incomparable_order": list(self.incomparable_order), "exchange_order": list(self.exchange_order), "checkpoint_seq": self.checkpoint_seq, "retry_count": self.retry_count, "consumed_budget_units": self.consumed_budget_units, "run_digest": self.run_digest, "telemetry_digest": self.telemetry_digest, "federation_digest": self.federation_digest, "replay_identity": self.replay_identity, "witness_order": list(self.witness_order), "counterexample_order": list(self.counterexample_order), "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})


def operate_multimodal_context_compilation(*, request_id: str, workspace_id: str, workflow_id: str, scope: str, goal: str, study_ids: Sequence[str], modalities: Sequence[str], cells: Sequence[MultimodalContextControlCell], max_retries: int, budget_units: int, replay_identity: str, policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True, signed_approval: bool = True) -> MultimodalContextControlReceipt:
    if not request_id.strip() or not workspace_id.strip() or not workflow_id.strip() or not scope.strip() or not goal.strip() or len(study_ids) < 2 or len(modalities) < 2 or not cells or budget_units <= 0 or max_retries < 0 or not re.fullmatch(r"[0-9a-f]{64}", replay_identity):
        raise ResearchContractError("multimodal control identity, matrix, budget, replay, or boundary is invalid")
    studies = tuple(sorted(set(study_ids))); modes = tuple(sorted(set(modalities)))
    if len(studies) != len(study_ids) or len(modes) != len(modalities) or any(not value.strip() for value in (*studies, *modes)):
        raise ResearchContractError("study and modality identifiers must be unique and non-empty")
    candidates = tuple(f"{study}|{modality}" for study in studies for modality in modes); cell_map: dict[str, MultimodalContextControlCell] = {}
    for cell in cells:
        key = f"{cell.study_id}|{cell.modality}"
        if key in cell_map: raise ResearchContractError("multimodal control cells must be unique")
        cell_map[key] = cell
    completed: set[str] = set(); degraded: set[str] = set(); unresolved: set[str] = set(); denied: set[str] = set(); incomparable: set[str] = set(); exchanges: list[str] = []
    witnesses = {"gate:typed-multimodal-control-contract", "gate:study-modality-closure", "gate:comparability", "gate:checkpoint", "gate:bounded-retry", "gate:telemetry", "gate:provenance", "gate:replay-identity", "gate:locality"}; counter: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); open_gate = policy_allow and protected_closure and raw_data_local and signed_approval; consumed = 0; retries = 0
    for key in candidates:
        cell = cell_map.get(key)
        if cell is None: unresolved.add(key); omissions.add(f"cell:{key}:missing-checkpoint")
        else:
            retries += cell.retry_count
            if not open_gate or not cell.raw_data_local or cell.boundary != PRECLINICAL_BOUNDARY: denied.add(key); counter.add(f"counterexample:{key}:policy-approval-locality")
            elif not cell.comparable: denied.add(key); incomparable.add(key); negative.add(f"cell:{key}:incomparable")
            elif cell.retry_count > max_retries: degraded.add(key); omissions.add(f"cell:{key}:retry-budget-exhausted")
            elif consumed + cell.cost_units > budget_units: denied.add(key); omissions.add(f"cell:{key}:resource-budget-exhausted")
            elif not cell.ready: unresolved.add(key); uncertainty.add(f"cell:{key}:not-ready")
            elif cell.replay_identity != replay_identity: unresolved.add(key); uncertainty.add(f"cell:{key}:replay-mismatch")
            elif cell.telemetry_digest is None: unresolved.add(key); omissions.add(f"cell:{key}:telemetry-missing")
            elif cell.evidence_digest is None or cell.provenance_digest is None: unresolved.add(key); omissions.add(f"cell:{key}:evidence-or-provenance-missing")
            elif cell.state in {"unknown", "speculative"}: unresolved.add(key); uncertainty.add(f"cell:{key}:evidence-uncertain")
            elif cell.state == "contradicted": denied.add(key); negative.add(f"cell:{key}:contradicted")
            else: completed.add(key); consumed += cell.cost_units; exchanges.append(research_artifact_digest({"cell_id": key, "context_digest": cell.context_digest, "section_digest": cell.section_digest, "evidence_digest": cell.evidence_digest, "provenance_digest": cell.provenance_digest, "telemetry_digest": cell.telemetry_digest}))
    if not policy_allow: counter.add("counterexample:policy-denied"); omissions.add("control:policy-denied")
    if not protected_closure: counter.add("counterexample:protected-closure-incomplete"); omissions.add("control:protected-closure-incomplete")
    if not signed_approval: counter.add("counterexample:signed-approval-missing"); omissions.add("control:signed-approval-missing")
    if unresolved or degraded: witnesses.add("gate:degraded-or-unresolved-retained")
    exchange_order = tuple(sorted(exchanges)); disposition = "denied" if not open_gate or denied else "unresolved" if unresolved else "degraded" if degraded else "completed"
    telemetry = research_artifact_digest({"feature_id": MULTIMODAL_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_FEATURE_ID, "workflow_id": workflow_id, "cell_order": list(candidates), "retry_count": retries, "exchange_order": list(exchange_order)}); federation = research_artifact_digest({"workspace_id": workspace_id, "workflow_id": workflow_id, "exchange_order": list(exchange_order), "raw_data_local": raw_data_local, "replay_identity": replay_identity}); run = research_artifact_digest({"feature_id": MULTIMODAL_CONTEXT_COMPILATION_FEDERATED_CONTROL_PLANE_FEATURE_ID, "request_id": request_id, "disposition": disposition, "completed_order": sorted(completed), "degraded_order": sorted(degraded), "unresolved_order": sorted(unresolved), "denied_order": sorted(denied), "checkpoint_seq": len(candidates), "consumed_budget_units": consumed, "telemetry_digest": telemetry, "federation_digest": federation, "replay_identity": replay_identity}); artifact = {"content_hash": research_artifact_digest({"request_id": request_id, "run_digest": run}), "media_type": "application/vnd.aurora.multimodal-context-control+json"}
    receipt = MultimodalContextControlReceipt(request_id=request_id, workspace_id=workspace_id, workflow_id=workflow_id, scope=scope, goal=goal, disposition=disposition, study_order=studies, modality_order=modes, cell_order=candidates, completed_order=tuple(sorted(completed)), degraded_order=tuple(sorted(degraded)), unresolved_order=tuple(sorted(unresolved)), denied_order=tuple(sorted(denied)), incomparable_order=tuple(sorted(incomparable)), exchange_order=exchange_order, checkpoint_seq=len(candidates), retry_count=retries, consumed_budget_units=consumed, run_digest=run, telemetry_digest=telemetry, federation_digest=federation, replay_identity=replay_identity, witness_order=tuple(sorted(witnesses)), counterexample_order=tuple(sorted(counter)), omissions=tuple(sorted(omissions)), uncertainty=tuple(sorted(uncertainty)), negative_evidence=tuple(sorted(negative)), effect_receipts=(f"exchange:permitted-multimodal-summary:{request_id}", f"manage:multimodal-context:{request_id}") if disposition == "completed" else ("block:unsafe-release",), artifact=artifact, raw_data_local=raw_data_local); receipt.validate(); return receipt
