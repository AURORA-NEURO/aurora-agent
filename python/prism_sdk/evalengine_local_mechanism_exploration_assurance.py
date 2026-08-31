"""Python parity surface for ``AFA-evalengine-P08-F25``."""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import re
from typing import Any, Mapping, Sequence

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-evalengine-P08-F25"
CONTRACT_VERSION = "evalengine-local-single-study-mechanism-exploration-assurance/1.0"
INPUT_SCHEMA = "MechanismQuestion1@1"
OUTPUT_SCHEMA = "MechanismPortfolio7@1"
CONTENT_TYPE = "application/vnd.aurora.mechanism-portfolio+json"

def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None

def _ordered(values: Sequence[str]) -> bool:
    return tuple(values) == tuple(sorted(set(values)))

def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()

@dataclass(frozen=True)
class EvalengineMechanismPortfolio7:
    value: Mapping[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return dict(self.value)

    def validate(self) -> None:
        v = self.value
        if (v.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or v.get("contract_version") != CONTRACT_VERSION or v.get("feature_id") != FEATURE_ID or v.get("boundary") != PRECLINICAL_BOUNDARY or v.get("raw_data_local") is not True or not all(str(v.get(k, "")).strip() for k in ("request_id", "consumer", "scope", "batch_id", "baseline_id", "algorithm_version")) or int(v.get("capacity", 0)) <= 0 or int(v.get("active_jobs", 0)) > int(v.get("capacity", 0)) or v.get("verdict") not in {"qualified", "conditional", "unknown", "blocked"} or not v.get("candidate_order")):
            raise ResearchContractError("mechanism portfolio identity, capacity, candidates, verdict, locality, or boundary is incomplete")
        fields = ("candidate_order", "admitted_order", "blocked_order", "unknown_order", "required_order", "check_order", "passed_checks", "counterexamples", "omissions", "uncertainty", "negative_evidence", "effect_receipts")
        if any(not _ordered(v.get(field, ())) for field in fields):
            raise ResearchContractError("mechanism portfolio ordering is not canonical")
        ids = set(v["candidate_order"])
        classified = set(v.get("admitted_order", ())) | set(v.get("blocked_order", ())) | set(v.get("unknown_order", ()))
        if any(x not in ids for x in classified):
            raise ResearchContractError("mechanism classifications reference an unknown candidate")
        for digest in (v.get("replay_identity"), v.get("assurance_digest"), v.get("artifact", {}).get("content_hash")):
            if not _digest(digest):
                raise ResearchContractError("mechanism portfolio digest is invalid")
        artifact = v.get("artifact", {})
        if artifact.get("content_type") != CONTENT_TYPE or artifact.get("boundary") != PRECLINICAL_BOUNDARY:
            raise ResearchContractError("mechanism portfolio artifact metadata is invalid")
        effects = v.get("effect_receipts", ())
        if any(effect != "block:unsafe-release" for effect in effects):
            raise ResearchContractError("mechanism portfolio effect is outside the unsafe-release gate")
        if v.get("verdict") != "qualified" and list(effects) != ["block:unsafe-release"]:
            raise ResearchContractError("non-qualified mechanism portfolio must block release")

def evalengine_local_mechanism_exploration_assurance_manifest() -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION, "owner_crate": "evalengine", "consumers": ["research program lead", "downstream AURORA crate maintainer", "independent validation partner"], "behavior": "deterministically verifies caller-supplied local single-study mechanism candidates without inventing mechanisms", "value": "prevents unsupported mechanistic explanations from entering release while retaining counterevidence", "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "effects": ["block:unsafe-release"], "permissions": ["evaluate:capability-runs"], "autonomy_tier": "A0", "boundary": PRECLINICAL_BOUNDARY}

def assure_evalengine_local_mechanism_exploration(*, request_id: str, consumer: str, scope: str, batch_id: str, baseline_id: str, algorithm_version: str, min_support_score: int, capacity: int, active_jobs: int, candidates: Sequence[Mapping[str, Any]], required_mechanism_ids: Sequence[str], policy_allow: bool, protected_closure: bool, signed_approval: bool, raw_data_local: bool, replay_identity: str, boundary: str = PRECLINICAL_BOUNDARY) -> EvalengineMechanismPortfolio7:
    if not all(str(x).strip() for x in (request_id, consumer, scope, batch_id, baseline_id, algorithm_version)) or capacity <= 0 or active_jobs > capacity or not candidates or not raw_data_local or not _digest(replay_identity) or boundary != PRECLINICAL_BOUNDARY or not _ordered(required_mechanism_ids):
        raise ResearchContractError("mechanism question identity, capacity, candidates, locality, replay, or boundary is invalid")
    rows = [dict(candidate) for candidate in candidates]; seen: set[str] = set()
    for row in rows:
        cid = str(row.get("candidate_id", ""))
        if (not cid.strip() or cid in seen or not str(row.get("mechanism_id", "")).strip() or str(row.get("scope", "")) != scope or not row.get("study_ids") or len(row["study_ids"]) != 1 or not row.get("modality_ids") or int(row.get("support_milli", 0)) > 1000 or row.get("boundary") != PRECLINICAL_BOUNDARY or not all(_digest(row.get(k)) for k in ("artifact_digest", "evidence_digest", "provenance_digest", "comparability_digest")) or not _ordered(row.get("omissions", ())) or not _ordered(row.get("uncertainty", ()) )):
            raise ResearchContractError(f"candidate {cid} is malformed or duplicated")
        seen.add(cid)
    rows.sort(key=lambda row: (-int(row.get("support_milli", 0)), str(row.get("mechanism_id", "")), str(row.get("candidate_id", ""))))
    candidate_order = [str(row["candidate_id"]) for row in rows]
    admitted: set[str] = set(); blocked: set[str] = set(); unknown: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); provenance: set[str] = set()
    for row in rows:
        cid = str(row["candidate_id"]); state = str(row.get("state", row.get("evidence_state", ""))).lower()
        if state == "contradicted":
            blocked.add(cid); omissions.add(f"candidate:{cid}:contradicted-evidence")
        elif state in {"unknown", "unmeasured"}:
            unknown.add(cid); uncertainty.add(f"candidate:{cid}:state-{state}-not-admitted")
        elif state == "supported" and int(row.get("support_milli", 0)) >= min_support_score and not row.get("omissions") and not row.get("uncertainty"):
            admitted.add(cid); provenance.add(str(row["provenance_digest"]))
        else:
            blocked.add(cid); omissions.add(f"candidate:{cid}:below-support-threshold")
        if row.get("negative_result"):
            negative.add(f"candidate:{cid}:negative-result-retained")
    for required in required_mechanism_ids:
        if required not in admitted:
            omissions.add(f"candidate:{required}:required-but-not-admitted")
    if not policy_allow or not protected_closure or not signed_approval:
        blocked.update(candidate_order); admitted.clear(); unknown.clear(); omissions.add("request:policy-protected-closure-or-approval-denied")
    verdict = "blocked" if not policy_allow or not raw_data_local else "unknown" if not admitted else "conditional" if blocked or unknown or omissions or uncertainty or not protected_closure else "qualified"
    checks = sorted(("baseline and algorithm binding", "candidate identities and canonical ordering", "negative and unknown evidence retention", "replay identity binding", "typed evidence, provenance, artifact, and comparability digests"))
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request_id, "consumer": consumer, "scope": scope, "batch_id": batch_id, "baseline_id": baseline_id, "algorithm_version": algorithm_version, "capacity": capacity, "active_jobs": active_jobs, "verdict": verdict, "candidate_order": candidate_order, "admitted_order": sorted(admitted), "blocked_order": sorted(blocked), "unknown_order": sorted(unknown), "required_order": list(required_mechanism_ids), "check_order": checks, "passed_checks": checks if verdict == "qualified" else [], "counterexamples": sorted(f"required mechanism not admitted: {x}" for x in required_mechanism_ids if x not in admitted), "omissions": sorted(omissions), "uncertainty": sorted(uncertainty), "negative_evidence": sorted(negative), "replay_identity": replay_identity, "raw_data_local": True, "boundary": PRECLINICAL_BOUNDARY}
    assurance_digest = _hash(payload); payload["assurance_digest"] = assurance_digest; payload["effect_receipts"] = [] if verdict == "qualified" else ["block:unsafe-release"]
    payload["artifact"] = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"mechanism-portfolio:{batch_id}", "content_type": CONTENT_TYPE, "content_hash": _hash({**payload, "artifact": None}), "semantic_loss": sorted(omissions), "provenance_digests": sorted(provenance), "boundary": PRECLINICAL_BOUNDARY}
    receipt = EvalengineMechanismPortfolio7(payload); receipt.validate(); return receipt

__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "EvalengineMechanismPortfolio7", "evalengine_local_mechanism_exploration_assurance_manifest", "assure_evalengine_local_mechanism_exploration"]
