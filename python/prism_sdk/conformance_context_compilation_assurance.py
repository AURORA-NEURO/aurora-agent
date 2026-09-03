"""Python parity surface for AFA-conformance-P03-F26.

The function is intentionally a validator/receipt builder: caller-supplied summaries are
partitioned deterministically and no source, instrument, network, or raw-data effect is run.
"""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import re
from typing import Any, Mapping, Sequence

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-conformance-P03-F26"
CONTRACT_VERSION = "conformance-multimodal-context-compilation-assurance/1.0"
INPUT_SCHEMA = "DecisionQuery2@1"
OUTPUT_SCHEMA = "CertifiedDecisionSection7@1"
CONTENT_TYPE = "application/vnd.aurora.certified-decision-section-7+json"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: Sequence[str]) -> bool:
    return list(values) == sorted(set(values))


@dataclass(frozen=True)
class CertifiedDecisionSection7:
    value: Mapping[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return dict(self.value)

    def validate(self) -> None:
        v = self.value
        required = ("request_id", "consumer", "federation_id", "purpose", "semantic_profile", "target_schema")
        if (v.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version") != CONTRACT_VERSION
                or v.get("feature_id") != FEATURE_ID or v.get("boundary") != PRECLINICAL_BOUNDARY
                or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True
                or any(not str(v.get(k, "")).strip() for k in required)
                or not v.get("fact_order") or not v.get("study_order") or not v.get("modality_order")
                or not v.get("peer_order") or v.get("effect_receipts") != ["block:unsafe-release"]):
            raise ResearchContractError("context identity, closure, locality, or release gate is incomplete")
        keys = ("fact_order", "selected_fact_order", "unresolved_fact_order", "blocked_fact_order", "missing_fact_order",
                "study_order", "selected_study_order", "missing_study_order", "modality_order", "selected_modality_order",
                "missing_modality_order", "peer_order", "qualified_peer_order", "missing_peer_order", "omission_order",
                "uncertainty_order", "negative_evidence_order", "effect_receipts")
        if any(not _ordered(v.get(k, ())) for k in keys):
            raise ResearchContractError("context ordering is not canonical")
        for all_ids, parts in ((v["fact_order"], v.get("selected_fact_order", ()) + v.get("unresolved_fact_order", ()) + v.get("blocked_fact_order", ()) + v.get("missing_fact_order", ())),
                               (v["study_order"], v.get("selected_study_order", ()) + v.get("missing_study_order", ())),
                               (v["modality_order"], v.get("selected_modality_order", ()) + v.get("missing_modality_order", ())),
                               (v["peer_order"], v.get("qualified_peer_order", ()) + v.get("missing_peer_order", ()) )):
            if set(all_ids) != set(parts) or len(parts) != len(set(parts)):
                raise ResearchContractError("context outcomes do not partition")
        artifact = v.get("artifact", {})
        if (not _digest(v.get("replay_identity")) or not _digest(v.get("section_digest"))
                or artifact.get("content_hash") != v.get("section_digest")
                or artifact.get("content_type") != CONTENT_TYPE or artifact.get("boundary") != PRECLINICAL_BOUNDARY):
            raise ResearchContractError("context artifact metadata or digest is invalid")

    def digest(self) -> str:
        self.validate()
        return _hash(self.value)


def assure_context_compilation(*, request: Mapping[str, Any], facts: Sequence[Mapping[str, Any]], peers: Sequence[Mapping[str, Any]]) -> CertifiedDecisionSection7:
    if (request.get("schema_version") != INPUT_SCHEMA or any(not str(request.get(k, "")).strip() for k in ("request_id", "consumer", "federation_id", "purpose", "semantic_profile", "target_schema"))
            or not _digest(request.get("replay_identity")) or int(request.get("budget_units", 0)) <= 0
            or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True
            or request.get("boundary") != PRECLINICAL_BOUNDARY or not facts or not peers):
        raise ResearchContractError("context identity, replay, budget, locality, or boundary is invalid")
    required = set(map(str, request.get("required_fact_order", ())))
    rows = sorted(facts, key=lambda f: (-int(f.get("influence_milli", 0)), str(f.get("fact_id", ""))))
    ids = sorted({str(f.get("fact_id", "")) for f in rows} | required)
    selected, unresolved, blocked, missing = set(), set(), set(), set(required - {str(f.get("fact_id", "")) for f in rows})
    studies, modalities, selected_studies, selected_modalities = set(), set(), set(), set()
    omissions, uncertainty, negative, provenance = set(), set(), set(), set()
    for f in rows:
        fid, sid, modality = str(f.get("fact_id", "")), str(f.get("study_id", "")), str(f.get("modality", ""))
        studies.add(sid); modalities.add(modality); provenance.add(str(f.get("provenance_digest", "")))
        omissions.update(f"{fid}:{x}" for x in f.get("omission_order", ())); uncertainty.update(f"{fid}:{x}" for x in f.get("uncertainty_order", ()))
        if f.get("negative_result") is True or str(f.get("evidence_state")) in {"contradicted", "negative"}: negative.add(f"{fid}:negative-result")
        if fid not in required: unresolved.add(fid)
        elif (not f.get("permitted") or not f.get("raw_data_local") or not f.get("aggregate_only")
              or str(f.get("semantic_profile")) != str(request["semantic_profile"])
              or str(f.get("replay_identity")) != str(request["replay_identity"])): blocked.add(fid)
        elif str(f.get("evidence_state")) in {"proven", "supported"} and int(f.get("influence_milli", 0)) >= 600:
            selected.add(fid); selected_studies.add(sid); selected_modalities.add(modality)
        else: unresolved.add(fid)
    qualified_peers, missing_peers = set(), set()
    for p in sorted(peers, key=lambda x: str(x.get("peer_id", ""))):
        pid = str(p.get("peer_id", ""))
        ok = (str(p.get("semantic_profile")) == str(request["semantic_profile"]) and p.get("signed") is True and p.get("permitted") is True
              and p.get("raw_data_local") is True and p.get("aggregate_only") is True and str(p.get("replay_identity")) == str(request["replay_identity"])
              and str(p.get("evidence_state")) in {"proven", "supported"})
        (qualified_peers if ok else missing_peers).add(pid)
        omissions.update(f"{pid}:{x}" for x in p.get("omission_order", ())); uncertainty.update(f"{pid}:{x}" for x in p.get("uncertainty_order", ()))
    global_block = (request.get("policy_allow") is not True or request.get("protected_closure") is not True
                    or bool(request.get("adversarial_event_order")))
    uncertainty.update(f"adversarial:{x}" for x in request.get("adversarial_event_order", ()))
    if global_block:
        blocked.update(ids); selected.clear(); unresolved.clear(); missing.clear(); omissions.add("request:governance-or-adversarial-blocked")
    for fid in sorted(required - selected): omissions.add(f"required:{fid}")
    selected_studies = selected_studies & studies; selected_modalities = selected_modalities & modalities
    required_studies, required_modalities, required_peers = map(set, (request.get("required_study_order", ()), request.get("required_modality_order", ()), request.get("required_peer_order", ())))
    missing_studies, missing_modalities = required_studies - selected_studies, required_modalities - selected_modalities
    disposition = "blocked" if global_block else ("unresolved" if unresolved or blocked or missing or missing_studies or missing_modalities or not required_peers <= qualified_peers else "qualified")
    if disposition != "qualified": omissions.add("request:context-closure-not-ready")
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID,
        "request_id": str(request["request_id"]), "consumer": str(request["consumer"]), "federation_id": str(request["federation_id"]), "purpose": str(request["purpose"]),
        "semantic_profile": str(request["semantic_profile"]), "target_schema": str(request["target_schema"]), "disposition": disposition,
        "fact_order": ids, "selected_fact_order": sorted(selected), "unresolved_fact_order": sorted(unresolved), "blocked_fact_order": sorted(blocked), "missing_fact_order": sorted(missing),
        "study_order": sorted(studies), "selected_study_order": sorted(selected_studies), "missing_study_order": sorted(missing_studies), "modality_order": sorted(modalities), "selected_modality_order": sorted(selected_modalities), "missing_modality_order": sorted(missing_modalities),
        "peer_order": sorted(str(p.get("peer_id", "")) for p in peers), "qualified_peer_order": sorted(qualified_peers), "missing_peer_order": sorted(missing_peers),
        "omission_order": sorted(omissions), "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative), "replay_identity": str(request["replay_identity"]), "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    digest = _hash(payload); payload["section_digest"] = digest; payload["artifact"] = {"artifact_id": f"certified-decision-section-7:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": digest, "semantic_loss": payload["omission_order"], "provenance_digests": sorted(provenance), "boundary": PRECLINICAL_BOUNDARY}; payload["effect_receipts"] = ["block:unsafe-release"]
    receipt = CertifiedDecisionSection7(payload); receipt.validate(); return receipt


def conformance_context_compilation_assurance_manifest() -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION, "owner_crate": "conformance", "consumers": ["consortium operator", "context compiler", "release reviewer"], "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "autonomy_tier": "A1", "effects": ["block:unsafe-release"], "boundary": PRECLINICAL_BOUNDARY}


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CertifiedDecisionSection7", "assure_context_compilation", "conformance_context_compilation_assurance_manifest"]

