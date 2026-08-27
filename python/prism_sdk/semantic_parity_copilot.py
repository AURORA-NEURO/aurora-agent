"""Python parity adapter for ``AFA-interweave-P28-F11``."""
from __future__ import annotations

from dataclasses import dataclass, asdict
import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-interweave-P28-F11"
CONTRACT_VERSION = "interweave-prospective-semantic-parity-research-copilot/1.0"
INPUT_SCHEMA = "InterweaveParityFixture3@1"
OUTPUT_SCHEMA = "InterweaveParityWitness3@1"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _canonical(values: list[str] | tuple[str, ...]) -> bool:
    return list(values) == sorted(set(values))


@dataclass(frozen=True)
class SemanticParityWitness:
    schema_version: str; contract_version: str; feature_id: str; fixture_id: str; batch_id: str; scope: str; disposition: str; parity_order: tuple[str, ...]; matched_order: tuple[str, ...]; missing_order: tuple[str, ...]; mismatch_order: tuple[str, ...]; uncertain_order: tuple[str, ...]; omission_order: tuple[str, ...]; invocation_receipt: str; replay_identity: str; witness_digest: str; semantic_loss: tuple[Mapping[str, Any], ...]; omissions: tuple[str, ...]; uncertainty: tuple[str, ...]; negative_evidence: tuple[str, ...]; artifact: Mapping[str, Any]; effect_receipts: tuple[str, ...]; raw_data_local: bool; boundary: str
    def validate(self) -> None:
        if (self.schema_version, self.contract_version, self.feature_id) != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID) or not all(str(value).strip() for value in (self.fixture_id, self.batch_id, self.scope, self.invocation_receipt)) or not self.parity_order or not self.effect_receipts or self.raw_data_local is not True or self.boundary != PRECLINICAL_BOUNDARY: raise ResearchContractError("parity identity, witness, locality, or effects are incomplete")
        for values in (self.parity_order, self.matched_order, self.missing_order, self.mismatch_order, self.uncertain_order, self.omission_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if not _canonical(values): raise ResearchContractError("parity ordering is not canonical")
        covered = list(self.matched_order) + list(self.missing_order) + list(self.mismatch_order) + list(self.uncertain_order)
        if set(covered) != set(self.parity_order) or len(covered) != len(set(covered)): raise ResearchContractError("parity states do not partition surfaces")
        if any(not effect.startswith("invoke:declared-tool:") and effect != "block:unsafe-release" for effect in self.effect_receipts): raise ResearchContractError("parity effect is outside declared-tool gate")
        if self.artifact.get("content_type") != "application/vnd.aurora.interweave-parity-witness+json" or not _digest(self.artifact.get("content_hash")): raise ResearchContractError("parity artifact type or digest is invalid")
    def to_dict(self) -> dict[str, Any]:
        value = asdict(self)
        for key, item in value.items():
            if isinstance(item, tuple): value[key] = list(item)
        return value


def compare_semantic_parity(*, request: Mapping[str, Any]) -> SemanticParityWitness:
    if request.get("schema_version") != INPUT_SCHEMA or not all(str(request.get(field, "")).strip() for field in ("fixture_id", "batch_id", "scope")) or not _digest(request.get("expected_canonical_digest")) or not _digest(request.get("artifact_digest")) or not _digest(request.get("provenance_digest")) or not _digest(request.get("replay_identity")) or int(request.get("budget_units", 0)) <= 0 or request.get("raw_data_local") is not True or request.get("boundary") != PRECLINICAL_BOUNDARY: raise ResearchContractError("parity fixture identity, budget, locality, replay, or boundary is invalid")
    order = ["python", "rust", "typescript"]; values = {"python": request.get("python_digest"), "rust": request.get("rust_digest"), "typescript": request.get("typescript_digest")}; matched: list[str] = []; missing: list[str] = []; mismatch: list[str] = []; uncertain: list[str] = []; omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); semantic_loss: list[Mapping[str, Any]] = []
    for surface in order:
        if values[surface] is None: missing.append(surface); omissions.add(f"{surface}:digest-missing")
        elif values[surface] == request["expected_canonical_digest"]: matched.append(surface); negative.add(f"{surface}:negative-result-not-observed")
        else: mismatch.append(surface); negative.add(f"{surface}:parity-mismatch"); semantic_loss.append({"field": f"surface:{surface}", "reason": "cross-language canonical digest mismatch", "severity": "decision_relevant"})
    state = str(request.get("evidence_state", "unknown"));
    if state in {"unknown", "speculative", "contradicted"}: uncertain = list(order); uncertainty.add(f"fixture:evidence-state:{state}")
    global_fail = request.get("policy_allow") is not True or request.get("protected_closure") is not True or bool(request.get("adversarial_events"));
    if request.get("policy_allow") is not True: omissions.add("fixture:policy-denied")
    if request.get("protected_closure") is not True: omissions.add("fixture:protected-closure-incomplete")
    if request.get("signed_approval") is not True: omissions.add("fixture:signed-approval-missing")
    omissions.update(f"fixture:adversarial:{event}" for event in request.get("adversarial_events", []))
    disposition = "blocked" if global_fail else "approval_required" if request.get("signed_approval") is not True else "unresolved" if mismatch or missing or uncertain else "qualified"; invocation = f"invoke:declared-tool:parity-batch:{request['batch_id']}" if disposition == "qualified" else "block:unsafe-release"; payload = {"schema_version": OUTPUT_SCHEMA, "fixture_id": request["fixture_id"], "batch_id": request["batch_id"], "parity_order": order, "matched_order": matched, "missing_order": missing, "mismatch_order": mismatch, "uncertain_order": uncertain, "replay_identity": request["replay_identity"], "disposition": disposition}; digest = _hash(payload); artifact = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"parity-witness:{request['fixture_id']}", "content_type": "application/vnd.aurora.interweave-parity-witness+json", "content_hash": digest, "semantic_loss": semantic_loss, "provenance": [{"source_id": request["batch_id"], "relation": "interweave-semantic-parity", "digest": digest}], "boundary": PRECLINICAL_BOUNDARY}; receipt = SemanticParityWitness(RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID, str(request["fixture_id"]), str(request["batch_id"]), str(request["scope"]), disposition, tuple(order), tuple(matched), tuple(missing), tuple(mismatch), tuple(uncertain), tuple(sorted(omissions)), invocation, str(request["replay_identity"]), digest, tuple(semantic_loss), tuple(sorted(omissions)), tuple(sorted(uncertainty)), tuple(sorted(negative)), artifact, (invocation,), True, PRECLINICAL_BOUNDARY); receipt.validate(); return receipt


__all__ = ["SemanticParityWitness", "compare_semantic_parity", "FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA"]
