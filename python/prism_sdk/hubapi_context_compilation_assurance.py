"""Python parity surface for ``AFA-hubapi-P03-F27``."""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import re
from typing import Any, Mapping, Sequence

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-hubapi-P03-F27"
CONTRACT_VERSION = "hubapi-prospective-context-compilation-assurance/1.0"
INPUT_SCHEMA = "DecisionQuery5@1"
OUTPUT_SCHEMA = "ContextAssuranceReport8@1"
CONTENT_TYPE = "application/vnd.aurora.hubapi-context-assurance-report+json"
CHECKPOINT_ORDER = ("admit-typed-query", "check-evidence-and-scope", "check-provenance-and-replay", "check-policy-and-federation", "retain-omission-and-negative-receipt")


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _canonical(values: Sequence[str]) -> bool:
    return tuple(values) == tuple(sorted(set(values)))


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


@dataclass(frozen=True)
class HubapiContextAssuranceReport:
    request_id: str; workflow_id: str; target_schema: str; purpose: str; semantic_profile: str; disposition: str
    fact_order: tuple[str, ...]; ranked_order: tuple[str, ...]; selected_order: tuple[str, ...]; unresolved_order: tuple[str, ...]; blocked_order: tuple[str, ...]
    missing_fact_order: tuple[str, ...]; missing_scope_order: tuple[str, ...]; omission_order: tuple[str, ...]; uncertainty_order: tuple[str, ...]; contradiction_order: tuple[str, ...]
    negative_evidence_order: tuple[str, ...]; adversarial_event_order: tuple[str, ...]; checkpoint_order: tuple[str, ...]; replay_identity: str; context_digest: str
    artifact: dict[str, Any]; effect_receipts: tuple[str, ...]; raw_data_local: bool = True; aggregate_only: bool = True
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION; contract_version: str = CONTRACT_VERSION; feature_id: str = FEATURE_ID; boundary: str = PRECLINICAL_BOUNDARY

    def to_dict(self) -> dict[str, Any]:
        return {"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "workflow_id": self.workflow_id, "target_schema": self.target_schema, "purpose": self.purpose, "semantic_profile": self.semantic_profile, "disposition": self.disposition, "fact_order": list(self.fact_order), "ranked_order": list(self.ranked_order), "selected_order": list(self.selected_order), "unresolved_order": list(self.unresolved_order), "blocked_order": list(self.blocked_order), "missing_fact_order": list(self.missing_fact_order), "missing_scope_order": list(self.missing_scope_order), "omission_order": list(self.omission_order), "uncertainty_order": list(self.uncertainty_order), "contradiction_order": list(self.contradiction_order), "negative_evidence_order": list(self.negative_evidence_order), "adversarial_event_order": list(self.adversarial_event_order), "checkpoint_order": list(self.checkpoint_order), "replay_identity": self.replay_identity, "context_digest": self.context_digest, "artifact": self.artifact, "effect_receipts": list(self.effect_receipts), "raw_data_local": self.raw_data_local, "aggregate_only": self.aggregate_only, "boundary": self.boundary}

    def validate(self) -> None:
        if (self.schema_version, self.contract_version, self.feature_id) != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID) or self.boundary != PRECLINICAL_BOUNDARY or self.artifact.get("boundary") != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.aggregate_only or not self.request_id.strip() or not self.workflow_id.strip() or not self.target_schema.strip() or not self.purpose.strip() or not self.semantic_profile.strip() or self.disposition not in {"qualified", "unresolved", "blocked"} or not self.fact_order or len(self.ranked_order) != len(self.fact_order) or not self.effect_receipts or self.checkpoint_order != CHECKPOINT_ORDER: raise ResearchContractError("context report identity, locality, ranking, checkpoints, disposition, or effects are incomplete")
        for values in (self.fact_order, self.selected_order, self.unresolved_order, self.blocked_order, self.missing_fact_order, self.missing_scope_order, self.omission_order, self.uncertainty_order, self.contradiction_order, self.negative_evidence_order, self.adversarial_event_order, self.effect_receipts):
            if not _canonical(values): raise ResearchContractError("context report ordering is not canonical")
        facts = set(self.fact_order); partitions = self.selected_order + self.unresolved_order + self.blocked_order
        if any(item not in facts for item in partitions) or len(partitions) != len(facts) or len(set(partitions)) != len(partitions) or any(item in facts for item in self.missing_fact_order) or set(self.ranked_order) != facts: raise ResearchContractError("context fact states do not partition observed facts")
        for value in (self.replay_identity, self.context_digest, self.artifact.get("content_hash")):
            if not _digest(value): raise ResearchContractError("context report digest is invalid")
        if self.artifact.get("content_type") != CONTENT_TYPE: raise ResearchContractError("context report artifact content type is invalid")
        if self.disposition == "qualified" and (len(self.effect_receipts) != 1 or not self.effect_receipts[0].startswith("verify:hubapi-context-assurance:")): raise ResearchContractError("qualified context report effect is invalid")
        if self.disposition != "qualified" and self.effect_receipts != ("block:unsafe-release",): raise ResearchContractError("non-qualified context report must block release")


def assure_context_compilation(*, request_id: str, workflow_id: str, target_schema: str, purpose: str, semantic_profile: str, required_fact_order: Sequence[str], required_scope_order: Sequence[str], facts: Sequence[Mapping[str, Any]], replay_identity: str, policy_allow: bool, protected_closure: bool, signed_approval: bool, federation_approved: bool, raw_data_local: bool, aggregate_only: bool, budget: int, max_budget: int, adversarial_events: Sequence[str] = (), boundary: str = PRECLINICAL_BOUNDARY) -> HubapiContextAssuranceReport:
    if not request_id.strip() or not workflow_id.strip() or not target_schema.strip() or not purpose.strip() or not semantic_profile.strip() or not facts or not required_fact_order or not required_scope_order or not _canonical(required_fact_order) or not _canonical(required_scope_order) or not _canonical(adversarial_events) or not _digest(replay_identity) or budget <= 0 or max_budget <= 0 or boundary != PRECLINICAL_BOUNDARY or not raw_data_local or not aggregate_only: raise ResearchContractError("query identity, required closure, digest, budget, locality, or boundary is invalid")
    rows = [dict(fact) for fact in facts]; seen: set[str] = set()
    for fact in rows:
        fid = str(fact.get("fact_id", ""))
        if not fid.strip() or not str(fact.get("proposition", "")).strip() or not str(fact.get("scope", "")).strip() or fid in seen or not 0 <= int(fact.get("influence_milli", -1)) <= 1000 or (fact.get("source_digest") is not None and not _digest(fact.get("source_digest"))) or (fact.get("provenance_digest") is not None and not _digest(fact.get("provenance_digest"))) or not _digest(fact.get("replay_identity")) or not str(fact.get("semantic_profile", "")).strip() or not _canonical(fact.get("omissions", ())) or not _canonical(fact.get("uncertainty", ())): raise ResearchContractError(f"fact {fid} is malformed or duplicated")
        seen.add(fid)
    rows.sort(key=lambda fact: (-int(fact["influence_milli"]), str(fact["fact_id"])))
    ranked_order = tuple(str(fact["fact_id"]) for fact in rows); fact_order = tuple(sorted(ranked_order)); fmap = {str(fact["fact_id"]): fact for fact in rows}; required = set(required_fact_order); scopes = set(required_scope_order)
    missing_fact_order = tuple(sorted(required - set(fmap))); selected: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); contradiction: set[str] = set(); negative: set[str] = set()
    for fact in rows:
        fid = str(fact["fact_id"]); state = str(fact.get("evidence_state", ""));
        if fact.get("negative_result"): negative.add(f"{fid}:negative-result")
        omissions.update(f"{fid}:{item}" for item in fact.get("omissions", ())); uncertainty.update(f"{fid}:{item}" for item in fact.get("uncertainty", ()))
        if state == "contradicted": blocked.add(fid); contradiction.add(f"{fid}:contradicted-evidence"); continue
        if state in {"unknown", "speculative"}: unresolved.add(fid); uncertainty.add(f"{fid}:evidence-unresolved"); continue
        complete = bool(str(fact.get("proposition", "")).strip()) and bool(fact.get("source_digest")) and bool(fact.get("provenance_digest")) and fact.get("semantic_profile") == semantic_profile and fact.get("replay_identity") == replay_identity and fact.get("scope") in scopes and not fact.get("omissions") and not fact.get("uncertainty") and bool(fact.get("local_data")) and bool(fact.get("permitted")) and int(fact["influence_milli"]) >= 500
        if complete and state in {"proven", "supported"}: selected.add(fid)
        else:
            unresolved.add(fid)
            if not fact.get("source_digest") or not fact.get("provenance_digest"): omissions.add(f"{fid}:source-or-provenance-missing")
            if fact.get("scope") not in scopes: omissions.add(f"{fid}:required-scope-missing")
            if int(fact["influence_milli"]) < 500: uncertainty.add(f"{fid}:influence-threshold-not-met")
            if not fact.get("local_data") or not fact.get("permitted"): blocked.add(fid); unresolved.discard(fid); omissions.add(f"{fid}:locality-or-permission-denied")
    omissions.update(f"{fid}:required-fact-missing" for fid in missing_fact_order); missing_scope_order = tuple(sorted(scope for scope in required_scope_order if not any(fact.get("scope") == scope for fact in rows))); omissions.update(f"required-scope-missing:{scope}" for scope in missing_scope_order); negative.update(f"adversarial:{event}" for event in adversarial_events)
    global_block = not policy_allow or not protected_closure or not signed_approval or not federation_approved or not raw_data_local or not aggregate_only or budget > max_budget or bool(adversarial_events); uncertainty.update(item for item in ("request:policy-denied",) if not policy_allow); uncertainty.update(item for item in ("request:protected-closure-incomplete",) if not protected_closure); uncertainty.update(item for item in ("request:institutional-approval-incomplete",) if not signed_approval or not federation_approved); omissions.update(item for item in ("request:budget-ceiling-exceeded",) if budget > max_budget)
    disposition = "blocked" if global_block else ("qualified" if not missing_fact_order and not missing_scope_order and selected and not unresolved and not blocked else "unresolved")
    selected_order, unresolved_order, blocked_order = tuple(sorted(selected)), tuple(sorted(unresolved)), tuple(sorted(blocked)); omission_order, uncertainty_order, contradiction_order, negative_order = tuple(sorted(omissions)), tuple(sorted(uncertainty)), tuple(sorted(contradiction)), tuple(sorted(negative)); effects = (f"verify:hubapi-context-assurance:{request_id}",) if disposition == "qualified" else ("block:unsafe-release",)
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request_id, "workflow_id": workflow_id, "target_schema": target_schema, "purpose": purpose, "semantic_profile": semantic_profile, "disposition": disposition, "fact_order": list(fact_order), "ranked_order": list(ranked_order), "selected_order": list(selected_order), "unresolved_order": list(unresolved_order), "blocked_order": list(blocked_order), "missing_fact_order": list(missing_fact_order), "missing_scope_order": list(missing_scope_order), "omission_order": list(omission_order), "uncertainty_order": list(uncertainty_order), "contradiction_order": list(contradiction_order), "negative_evidence_order": list(negative_order), "adversarial_event_order": list(adversarial_events), "checkpoint_order": list(CHECKPOINT_ORDER), "replay_identity": replay_identity, "effect_receipts": list(effects), "raw_data_local": raw_data_local, "aggregate_only": aggregate_only, "boundary": PRECLINICAL_BOUNDARY}
    context_digest = _hash(payload); artifact = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"hubapi-context-assurance:{request_id}", "content_type": CONTENT_TYPE, "content_hash": context_digest, "semantic_loss": [], "provenance": [{"source_id": f"workflow:{workflow_id}", "relation": "derived-from-local-context-manifest", "digest": replay_identity}], "boundary": PRECLINICAL_BOUNDARY}
    report = HubapiContextAssuranceReport(request_id=request_id, workflow_id=workflow_id, target_schema=target_schema, purpose=purpose, semantic_profile=semantic_profile, disposition=disposition, fact_order=fact_order, ranked_order=ranked_order, selected_order=selected_order, unresolved_order=unresolved_order, blocked_order=blocked_order, missing_fact_order=missing_fact_order, missing_scope_order=missing_scope_order, omission_order=omission_order, uncertainty_order=uncertainty_order, contradiction_order=contradiction_order, negative_evidence_order=negative_order, adversarial_event_order=tuple(adversarial_events), checkpoint_order=CHECKPOINT_ORDER, replay_identity=replay_identity, context_digest=context_digest, artifact=artifact, effect_receipts=effects)
    report.validate(); return report


def hubapi_context_assurance_digest(report: HubapiContextAssuranceReport) -> str:
    report.validate(); return _hash(report.to_dict())


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "HubapiContextAssuranceReport", "assure_context_compilation", "hubapi_context_assurance_digest"]
