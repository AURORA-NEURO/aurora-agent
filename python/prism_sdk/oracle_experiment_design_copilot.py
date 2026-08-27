"""Python parity surface for ``AFA-oracle-P09-F11``."""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
)

FEATURE_ID = "AFA-oracle-P09-F11"
CONTRACT_VERSION = "oracle-prospective-experiment-design-research-copilot/1.0"
INPUT_SCHEMA = "ExperimentObjective3@1"
OUTPUT_SCHEMA = "ExecutableExperimentDesign3@1"
CONTENT_TYPE = "application/vnd.aurora.executable-experiment-design-3+json"
MAX_PLAN_STEPS = 64


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _canonical(values: Sequence[str]) -> bool:
    return tuple(values) == tuple(sorted(set(values)))


def _hash(value: Any) -> str:
    return hashlib.sha256(
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()
    ).hexdigest()


@dataclass(frozen=True)
class OracleExperimentDesignCopilotReceipt:
    request_id: str
    operator_id: str
    workflow_id: str
    benchmark_id: str
    purpose: str
    scope: str
    semantic_profile: str
    disposition: str
    candidate_order: tuple[str, ...]
    ranked_order: tuple[str, ...]
    admitted_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    missing_candidate_order: tuple[str, ...]
    missing_factor_order: tuple[str, ...]
    plan_order: tuple[str, ...]
    action_order: tuple[str, ...]
    tool_order: tuple[str, ...]
    baseline_digest: str
    replay_identity: str
    plan_digest: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: dict[str, Any]
    budget_units: int
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    contract_version: str = CONTRACT_VERSION
    feature_id: str = FEATURE_ID
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "contract_version": self.contract_version,
            "feature_id": self.feature_id,
            "request_id": self.request_id,
            "operator_id": self.operator_id,
            "workflow_id": self.workflow_id,
            "benchmark_id": self.benchmark_id,
            "purpose": self.purpose,
            "scope": self.scope,
            "semantic_profile": self.semantic_profile,
            "disposition": self.disposition,
            "candidate_order": list(self.candidate_order),
            "ranked_order": list(self.ranked_order),
            "admitted_order": list(self.admitted_order),
            "unknown_order": list(self.unknown_order),
            "blocked_order": list(self.blocked_order),
            "missing_candidate_order": list(self.missing_candidate_order),
            "missing_factor_order": list(self.missing_factor_order),
            "plan_order": list(self.plan_order),
            "action_order": list(self.action_order),
            "tool_order": list(self.tool_order),
            "baseline_digest": self.baseline_digest,
            "replay_identity": self.replay_identity,
            "plan_digest": self.plan_digest,
            "omissions": list(self.omissions),
            "uncertainty": list(self.uncertainty),
            "negative_evidence": list(self.negative_evidence),
            "effect_receipts": list(self.effect_receipts),
            "artifact": self.artifact,
            "budget_units": self.budget_units,
            "raw_data_local": self.raw_data_local,
            "boundary": self.boundary,
        }

    def validate(self) -> None:
        if (
            (self.schema_version, self.contract_version, self.feature_id)
            != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID)
            or self.boundary != PRECLINICAL_BOUNDARY
            or not self.raw_data_local
            or not all(
                value.strip()
                for value in (
                    self.request_id,
                    self.operator_id,
                    self.workflow_id,
                    self.benchmark_id,
                    self.purpose,
                    self.scope,
                    self.semantic_profile,
                )
            )
            or not self.candidate_order
            or len(self.ranked_order) != len(self.candidate_order)
            or not self.plan_order
            or len(self.plan_order) != len(self.action_order)
            or not self.tool_order
            or not self.effect_receipts
            or self.budget_units <= 0
        ):
            raise ResearchContractError("copilot identity, bounded plan, locality, budget, or effects are incomplete")
        for values in (
            self.candidate_order,
            self.admitted_order,
            self.unknown_order,
            self.blocked_order,
            self.missing_candidate_order,
            self.missing_factor_order,
            self.plan_order,
            self.action_order,
            self.tool_order,
            self.omissions,
            self.uncertainty,
            self.negative_evidence,
            self.effect_receipts,
        ):
            if not _canonical(values):
                raise ResearchContractError("copilot ordering is not canonical")
        ids = set(self.candidate_order)
        partitions = self.admitted_order + self.unknown_order + self.blocked_order
        if (
            len(partitions) != len(ids)
            or any(value not in ids for value in partitions)
            or len(set(partitions)) != len(partitions)
            or set(self.ranked_order) != ids
        ):
            raise ResearchContractError("copilot candidate states do not partition candidates")
        for value in (
            self.baseline_digest,
            self.replay_identity,
            self.plan_digest,
            self.artifact.get("content_hash"),
        ):
            if not _digest(value):
                raise ResearchContractError("copilot digest is invalid")
        if self.artifact.get("content_type") != CONTENT_TYPE:
            raise ResearchContractError("copilot artifact type is invalid")
        if self.disposition == "qualified":
            if len(self.effect_receipts) != 1 or not self.effect_receipts[0].startswith("invoke:declared-tool:"):
                raise ResearchContractError("qualified copilot effect is invalid")
        elif self.effect_receipts != ("block:unsafe-release",):
            raise ResearchContractError("non-qualified copilot must block release")


def _state_rank(state: str) -> int:
    return {"proven": 4, "supported": 3, "speculative": 2, "unknown": 1, "contradicted": 0}.get(state, 0)


def compile_experiment_design_copilot(
    *,
    request_id: str,
    operator_id: str,
    workflow_id: str,
    benchmark_id: str,
    purpose: str,
    scope: str,
    semantic_profile: str,
    required_candidate_order: Sequence[str],
    required_factor_order: Sequence[str],
    baseline_digest: str,
    candidates: Sequence[Mapping[str, Any]],
    replay_identity: str,
    declared_tool_id: str,
    action_allow_list: Sequence[str],
    max_actions: int,
    budget_units: int,
    policy_allow: bool,
    protected_closure: bool,
    signed_approval: bool,
    raw_data_local: bool,
    boundary: str = PRECLINICAL_BOUNDARY,
) -> OracleExperimentDesignCopilotReceipt:
    if (
        not all(value.strip() for value in (request_id, operator_id, workflow_id, benchmark_id, purpose, scope, semantic_profile, declared_tool_id))
        or not required_candidate_order
        or not required_factor_order
        or not candidates
        or not action_allow_list
        or max_actions <= 0
        or max_actions > MAX_PLAN_STEPS
        or budget_units <= 0
        or boundary != PRECLINICAL_BOUNDARY
        or not raw_data_local
        or not _canonical(required_candidate_order)
        or not _canonical(required_factor_order)
        or not _digest(baseline_digest)
        or not _digest(replay_identity)
    ):
        raise ResearchContractError("copilot identity, scope, tool, bounds, digests, locality, or boundary is incomplete")
    rows = [dict(row) for row in candidates]
    seen: set[str] = set()
    for row in rows:
        identifier = str(row.get("candidate_id", ""))
        if (
            not identifier.strip()
            or identifier in seen
            or not str(row.get("label", "")).strip()
            or not row.get("factor_order")
            or not _canonical(row["factor_order"])
            or not 0 <= int(row.get("power_milli", -1)) <= 1000
            or not 0 <= int(row.get("replication_milli", -1)) <= 1000
            or int(row.get("expected_cost_units", 0)) <= 0
            or not _digest(row.get("design_digest"))
            or not _digest(row.get("baseline_digest"))
            or not _digest(row.get("provenance_digest"))
            or not _digest(row.get("replay_identity"))
            or not str(row.get("semantic_profile", "")).strip()
            or not _canonical(row.get("omissions", ()))
            or not _canonical(row.get("uncertainty", ()))
            or not _canonical(row.get("negative_evidence", ()))
        ):
            raise ResearchContractError(f"candidate {identifier} is malformed or duplicated")
        seen.add(identifier)
    rows.sort(key=lambda row: (-_state_rank(str(row["evidence_state"])), -(int(row["power_milli"]) + int(row["replication_milli"])), str(row["candidate_id"])))
    ranked = tuple(str(row["candidate_id"]) for row in rows)
    order = tuple(sorted(ranked))
    required = set(required_candidate_order)
    missing_candidate = tuple(sorted(required - set(order)))
    factors = set(required_factor_order)
    admitted: set[str] = set()
    unknown: set[str] = set()
    blocked: set[str] = set()
    omissions: set[str] = set()
    uncertainty: set[str] = set()
    negative: set[str] = set()
    spent = 0
    action_ok = "generate-experiment-design" in action_allow_list
    for row in rows:
        identifier = str(row["candidate_id"])
        omissions.update(f"{identifier}:{item}" for item in row.get("omissions", ()))
        uncertainty.update(f"{identifier}:{item}" for item in row.get("uncertainty", ()))
        negative.update(f"{identifier}:{item}" for item in row.get("negative_evidence", ()))
        state = str(row["evidence_state"])
        if state == "contradicted":
            blocked.add(identifier)
            negative.add(f"{identifier}:contradicted-evidence")
            continue
        if state in {"unknown", "speculative"}:
            unknown.add(identifier)
            uncertainty.add(f"{identifier}:evidence-unresolved")
            continue
        row_factors = set(row["factor_order"])
        budget_ok = int(row["expected_cost_units"]) <= budget_units - spent
        valid = (
            int(row["sample_size"]) > 0
            and int(row["power_milli"]) >= 800
            and int(row["replication_milli"]) >= 750
            and row["baseline_digest"] == baseline_digest
            and row["replay_identity"] == replay_identity
            and row["semantic_profile"] == semantic_profile
            and factors <= row_factors
            and not row.get("omissions")
            and not row.get("uncertainty")
            and not row.get("negative_evidence")
            and bool(row.get("local_data"))
            and budget_ok
            and action_ok
        )
        if valid and state in {"proven", "supported"}:
            spent += int(row["expected_cost_units"])
            admitted.add(identifier)
        else:
            unknown.add(identifier)
            if int(row["sample_size"]) == 0:
                omissions.add(f"{identifier}:sample-size-missing")
            if int(row["power_milli"]) < 800:
                uncertainty.add(f"{identifier}:power-threshold-not-met")
            if int(row["replication_milli"]) < 750:
                uncertainty.add(f"{identifier}:replication-threshold-not-met")
            if row["baseline_digest"] != baseline_digest:
                omissions.add(f"{identifier}:baseline-mismatch")
            if row["replay_identity"] != replay_identity:
                omissions.add(f"{identifier}:replay-mismatch")
            if not factors <= row_factors:
                omissions.add(f"{identifier}:factor-closure-incomplete")
            if not row.get("local_data"):
                blocked.add(identifier)
                unknown.discard(identifier)
                omissions.add(f"{identifier}:locality-denied")
            if not budget_ok:
                omissions.add(f"{identifier}:budget-ceiling-exceeded")
            if not action_ok:
                blocked.add(identifier)
                unknown.discard(identifier)
                negative.add(f"{identifier}:declared-tool-action-denied")
    omissions.update(f"{identifier}:required-candidate-missing" for identifier in missing_candidate)
    missing_factor = tuple(sorted(factor for factor in required_factor_order if not any(factor in row["factor_order"] for row in rows)))
    omissions.update(f"required-factor-missing:{factor}" for factor in missing_factor)
    if not policy_allow:
        negative.add("request:policy-denied")
    if not protected_closure:
        uncertainty.add("request:protected-closure-incomplete")
    if not signed_approval:
        uncertainty.add("request:signed-approval-missing")
    if not raw_data_local:
        negative.add("request:raw-data-locality-required")
    plan = {f"plan:review-design:{identifier}" for identifier in order}
    actions = {f"action:review-design:{identifier}" for identifier in order}
    if not admitted:
        plan.add("plan:retain-unresolved-designs")
        actions.add("action:retain-unresolved-designs")
    plan_order = tuple(sorted(plan))
    action_order = tuple(sorted(actions))
    global_block = not policy_allow or not protected_closure or not signed_approval or not raw_data_local or not action_ok
    qualified = not global_block and not missing_candidate and not missing_factor and bool(admitted) and not unknown and not blocked and len(plan_order) <= max_actions and len(plan_order) <= budget_units
    if qualified:
        disposition = "qualified"
    elif global_block or len(plan_order) > max_actions or len(plan_order) > budget_units:
        disposition = "blocked"
    elif not admitted:
        disposition = "unknown"
    else:
        disposition = "partial"
    admitted_order = tuple(sorted(admitted))
    unknown_order = tuple(sorted(unknown))
    blocked_order = tuple(sorted(blocked))
    effects = (f"invoke:declared-tool:{declared_tool_id}",) if disposition == "qualified" else ("block:unsafe-release",)
    tool_order = (declared_tool_id,)
    payload = {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request_id,
        "operator_id": operator_id,
        "workflow_id": workflow_id,
        "benchmark_id": benchmark_id,
        "purpose": purpose,
        "scope": scope,
        "semantic_profile": semantic_profile,
        "disposition": disposition,
        "candidate_order": list(order),
        "ranked_order": list(ranked),
        "admitted_order": list(admitted_order),
        "unknown_order": list(unknown_order),
        "blocked_order": list(blocked_order),
        "missing_candidate_order": list(missing_candidate),
        "missing_factor_order": list(missing_factor),
        "plan_order": list(plan_order),
        "action_order": list(action_order),
        "tool_order": list(tool_order),
        "baseline_digest": baseline_digest,
        "replay_identity": replay_identity,
        "omissions": sorted(omissions),
        "uncertainty": sorted(uncertainty),
        "negative_evidence": sorted(negative),
        "effect_receipts": list(effects),
        "budget_units": budget_units,
        "boundary": PRECLINICAL_BOUNDARY,
    }
    plan_digest = _hash(payload)
    artifact = {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "artifact_id": f"oracle-experiment-design:{request_id}",
        "content_type": CONTENT_TYPE,
        "content_hash": plan_digest,
        "semantic_loss": [],
        "provenance": [],
        "boundary": PRECLINICAL_BOUNDARY,
    }
    result = OracleExperimentDesignCopilotReceipt(
        request_id=request_id,
        operator_id=operator_id,
        workflow_id=workflow_id,
        benchmark_id=benchmark_id,
        purpose=purpose,
        scope=scope,
        semantic_profile=semantic_profile,
        disposition=disposition,
        candidate_order=order,
        ranked_order=ranked,
        admitted_order=admitted_order,
        unknown_order=unknown_order,
        blocked_order=blocked_order,
        missing_candidate_order=missing_candidate,
        missing_factor_order=missing_factor,
        plan_order=plan_order,
        action_order=action_order,
        tool_order=tool_order,
        baseline_digest=baseline_digest,
        replay_identity=replay_identity,
        plan_digest=plan_digest,
        omissions=tuple(sorted(omissions)),
        uncertainty=tuple(sorted(uncertainty)),
        negative_evidence=tuple(sorted(negative)),
        effect_receipts=effects,
        artifact=artifact,
        budget_units=budget_units,
    )
    result.validate()
    return result


def oracle_experiment_design_copilot_digest(result: OracleExperimentDesignCopilotReceipt) -> str:
    result.validate()
    return _hash(result.to_dict())


__all__ = [
    "FEATURE_ID",
    "CONTRACT_VERSION",
    "INPUT_SCHEMA",
    "OUTPUT_SCHEMA",
    "OracleExperimentDesignCopilotReceipt",
    "compile_experiment_design_copilot",
    "oracle_experiment_design_copilot_digest",
]
