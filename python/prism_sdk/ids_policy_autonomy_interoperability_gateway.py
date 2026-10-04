"""Python parity for ``AFA-ids-P19-F24`` policy/autonomy admission."""
from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-ids-P19-F24"
CONTRACT_VERSION = "ids-federated-policy-autonomy-interoperability-gateway/1.0"
INPUT_SCHEMA = "AutonomyPolicyRequest7@1"
OUTPUT_SCHEMA = "AutonomyPolicyReceipt9@1"
CONTENT_TYPE = "application/vnd.aurora.autonomy-policy-receipt-9+json"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


@dataclass(frozen=True)
class AutonomyPolicyReceipt9:
    value: dict[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return dict(self.value)

    def validate(self) -> None:
        v = self.value
        if v.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version") != CONTRACT_VERSION or v.get("feature_id") != FEATURE_ID or v.get("boundary") != PRECLINICAL_BOUNDARY or v.get("artifact", {}).get("boundary") != PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True or not v.get("actor_order") or not v.get("requested_action_order") or not v.get("effect_receipts") or v.get("disposition") not in {"qualified", "unresolved", "blocked"}:
            raise ResearchContractError("policy identity, locality, actors, actions, disposition, or effects are incomplete")
        fields = ("actor_order", "admitted_actor_order", "approval_required_actor_order", "denied_actor_order", "revoked_actor_order", "over_budget_actor_order", "scope_mismatch_order", "missing_authority_order", "requested_action_order", "permitted_action_order", "denied_action_order", "omission_order", "uncertainty_order", "negative_evidence_order", "effect_receipts")
        if any(not _ordered(v.get(k, [])) for k in fields):
            raise ResearchContractError("policy/autonomy ordering is not canonical")
        ids = set(v["actor_order"]); states = v["admitted_actor_order"] + v["approval_required_actor_order"] + v["denied_actor_order"]
        if len(ids) != len(v["actor_order"]) or len(states) != len(ids) or set(states) != ids:
            raise ResearchContractError("actor states do not partition")
        a = v.get("artifact", {}); digests = [v.get("replay_identity"), v.get("policy_digest"), a.get("content_hash"), *a.get("provenance_digests", [])]
        if not all(_digest(d) for d in digests) or a.get("content_type") != CONTENT_TYPE or a.get("content_hash") != v.get("policy_digest"):
            raise ResearchContractError("policy digest or artifact metadata is inconsistent")
        if any(not e.startswith(("exchange:policy-receipts:", "manage:autonomy-grant:")) and e != "block:unsafe-release" for e in v["effect_receipts"]):
            raise ResearchContractError("policy effect is outside governed gate")


def policy_autonomy_interoperability_manifest() -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION, "owner_crate": "ids", "consumers": ["research workflow operator", "institutional policy steward", "federation administrator", "autonomy auditor"], "behavior": "admit typed actor scopes and action budgets under risk-tiered federated policy", "value": "prevents revoked, over-scoped, over-budget, or under-authorized autonomy from crossing a research boundary", "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "effects": ["exchange:policy-receipts", "manage:autonomy-grant"], "permissions": ["read:local-authority-manifests", "request:autonomy-admission"], "autonomy_tier": "A2", "boundary": PRECLINICAL_BOUNDARY}


def _validate_request(request: Mapping[str, Any]) -> None:
    if not all(isinstance(request.get(k), str) and request[k].strip() for k in ("request_id", "purpose", "semantic_profile", "scope", "policy_version")) or int(request.get("required_tier", 9)) > 4 or not request.get("requested_actions") or not request.get("actors") or len(request["actors"]) > 1024 or not _digest(request.get("replay_identity")) or request.get("boundary") != PRECLINICAL_BOUNDARY or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True:
        raise ResearchContractError("policy identity, scope, tier, actions, actor bound, replay, or locality is invalid")
    ids: set[str] = set()
    for actor in request["actors"]:
        if not all(isinstance(actor.get(k), str) and actor[k].strip() for k in ("actor_id", "role", "authority", "scope")) or int(actor.get("autonomy_tier", 9)) > 4 or not actor.get("permitted_actions") or not _digest(actor.get("approval_digest")) or actor.get("actor_id") in ids:
            raise ResearchContractError("actor identity, authority, tier, actions, approval, or uniqueness is invalid")
        ids.add(actor["actor_id"])


def admit_policy_autonomy(request: Mapping[str, Any]) -> AutonomyPolicyReceipt9:
    _validate_request(request)
    actors = sorted((dict(a) for a in request["actors"]), key=lambda a: a["actor_id"]); ids = [a["actor_id"] for a in actors]
    admitted: set[str] = set(); approval: set[str] = set(); denied: set[str] = set(); revoked: set[str] = set(); over_budget: set[str] = set(); scope_mismatch: set[str] = set(); missing_authority: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); permitted: set[str] = set(); provenance: set[str] = set(); budget = 0
    for actor in actors:
        aid = actor["actor_id"]; provenance.add(actor["approval_digest"])
        if actor.get("revoked") is True: revoked.add(aid); denied.add(aid); negative.add(f"{aid}:revoked")
        elif int(actor.get("autonomy_tier", 9)) > int(request["required_tier"]): denied.add(aid); omissions.add(f"{aid}:autonomy-tier-exceeds-request")
        elif actor["scope"] != request["scope"]: scope_mismatch.add(aid); denied.add(aid); omissions.add(f"{aid}:scope-mismatch")
        elif not actor.get("authority"): missing_authority.add(aid); approval.add(aid); uncertainty.add(f"{aid}:authority-missing")
        elif int(actor.get("budget_units", 0)) <= 0: over_budget.add(aid); denied.add(aid); omissions.add(f"{aid}:budget-exhausted")
        else: admitted.add(aid); budget += int(actor["budget_units"]); permitted.update(set(actor["permitted_actions"]) & set(request["requested_actions"]))
    denied_actions = set(request["requested_actions"]) - permitted
    global_block = not all(request.get(k) is True for k in ("policy_allow", "protected_closure", "federation_approved", "raw_data_local", "aggregate_only"))
    if global_block: denied.update(ids); admitted.clear(); permitted.clear(); denied_actions.update(request["requested_actions"]); omissions.add("request:policy-protected-closure-or-federation-denied")
    ao, po, do = sorted(admitted), sorted(approval), sorted(denied); pacts, dacts = sorted(permitted), sorted(denied_actions); disposition = "blocked" if global_block or (not pacts and not po) else "unresolved" if po or do or dacts else "qualified"
    if disposition != "qualified": omissions.add("request:autonomy-admission-not-closed")
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "purpose": request["purpose"], "semantic_profile": request["semantic_profile"], "scope": request["scope"], "policy_version": request["policy_version"], "required_tier": int(request["required_tier"]), "disposition": disposition, "actor_order": ids, "admitted_actor_order": ao, "approval_required_actor_order": po, "denied_actor_order": do, "revoked_actor_order": sorted(revoked), "over_budget_actor_order": sorted(over_budget), "scope_mismatch_order": sorted(scope_mismatch), "missing_authority_order": sorted(missing_authority), "requested_action_order": list(request["requested_actions"]), "permitted_action_order": pacts, "denied_action_order": dacts, "omission_order": sorted(omissions), "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative), "budget_remaining": budget, "replay_identity": request["replay_identity"], "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    pd = _hash(payload); payload["policy_digest"] = pd; payload["artifact"] = {"artifact_id": f"autonomy-policy-receipt-9:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": pd, "semantic_loss": sorted(omissions), "provenance_digests": sorted(provenance), "boundary": PRECLINICAL_BOUNDARY}; payload["effect_receipts"] = sorted([f"exchange:policy-receipts:{request['request_id']}", f"manage:autonomy-grant:{request['request_id']}"] if disposition == "qualified" else ["block:unsafe-release"])
    receipt = AutonomyPolicyReceipt9(payload); receipt.validate(); return receipt


def idsPolicyAutonomyDigest(receipt: AutonomyPolicyReceipt9) -> str:
    receipt.validate(); return _hash(receipt.to_dict())


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "AutonomyPolicyReceipt9", "policy_autonomy_interoperability_manifest", "admit_policy_autonomy", "idsPolicyAutonomyDigest"]
