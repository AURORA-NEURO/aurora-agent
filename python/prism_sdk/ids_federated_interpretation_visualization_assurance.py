"""Python parity surface for the ids federated interpretation and visualization assurance contract."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
    
     ResearchContractError,
    research_artifact_digest,
)

FEATURE_ID = "AFA-ids-P14-F28"
CONTRACT_VERSION = "ids-federated-continual-interpretation-visualization-assurance/1.0"
INPUT_SCHEMA = "EvidenceBackedResult4@1"
OUTPUT_SCHEMA = "InteractiveInterpretation7@1"
CONTENT_TYPE = "application/vnd.aurora.ids-interactive-interpretation-7+json"


@dataclass(frozen=True)
class IdsInteractiveInterpretation7:
    value: Mapping[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return dict(self.value)

    def validate(self) -> None:
        v = self.value
        if (v.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION
                or v.get("contract_version") != CONTRACT_VERSION
                or v.get("feature_id") != FEATURE_ID
                or v.get("boundary") != PRECLINICAL_BOUNDARY
                or v.get("raw_data_local") is not True or v.get("aggregate_only") is not True
                or not v.get("effect_receipts")
                or (v.get("disposition") == "qualified" and not any(str(e).startswith("verify:ids-interpretation:") for e in v.get("effect_receipts", ())))
                or (v.get("disposition") != "qualified" and v.get("effect_receipts") != ["block:unsafe-release"])
                or not str(v.get("request_id", "")).strip() or not v.get("candidate_order")
                or v.get("artifact", {}).get("content_type") != CONTENT_TYPE):
            raise ResearchContractError("runtime interpretation identity, locality, artifact, or release gate is incomplete")
        ids = list(v["candidate_order"])
        parts = list(v.get("qualified_order", ())) + list(v.get("unresolved_order", ())) + list(v.get("blocked_order", ())) + list(v.get("incomparable_order", ()))
        if len(set(ids)) != len(ids) or len(parts) != len(ids) or set(parts) != set(ids):
            raise ResearchContractError("runtime interpretation states do not partition candidates")
        for digest in (v.get("replay_identity"), v.get("interpretation_digest"), v.get("artifact", {}).get("content_hash")):
            if not isinstance(digest, str) or not re.fullmatch(r"[0-9a-f]{64}", digest):
                raise ResearchContractError("runtime interpretation digest is invalid")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest(self.value)


def assure_ids_interpretation(request: Mapping[str, Any]) -> IdsInteractiveInterpretation7:
    if (request.get("schema_version") != INPUT_SCHEMA or request.get("boundary") != PRECLINICAL_BOUNDARY
            or not all(str(request.get(k, "")).strip() for k in ("request_id", "researcher", "purpose", "semantic_profile"))
            or not re.fullmatch(r"[0-9a-f]{64}", str(request.get("replay_identity", "")))
            or not re.fullmatch(r"[0-9a-f]{64}", str(request.get("comparability_digest", "")))
            or not request.get("raw_data_local") or not request.get("aggregate_only")
            or not isinstance(request.get("candidates"), Sequence) or not request["candidates"]):
        raise ResearchContractError("interpretation identity, digests, locality, or boundary is invalid")
    rows = sorted(request["candidates"], key=lambda c: (-int(c.get("interpretation_score_milli", 0)), str(c.get("candidate_id", ""))))
    ids = [str(c.get("candidate_id", "")) for c in rows]
    if len(set(ids)) != len(ids) or any(not i for i in ids):
        raise ResearchContractError("candidate identifiers must be unique and non-empty")
    qualified: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set(); incomparable: set[str] = set(); missing_study: set[str] = set(); missing_modality: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); provenance: set[str] = set()
    required_study = list(request.get("required_study_order", ())); required_modality = list(request.get("required_modality_order", ()))
    for c in rows:
        cid = str(c["candidate_id"]); provenance.add(str(c.get("provenance_digest", ""))); omissions.update(f"{cid}:{x}" for x in c.get("omission_order", ())); uncertainty.update(f"{cid}:{x}" for x in c.get("uncertainty_order", ()))
        if c.get("negative_result") or c.get("evidence_state") == "negative": negative.add(f"{cid}:negative-result")
        studies = list(c.get("study_order", ())); modalities = list(c.get("modality_order", ()))
        if any(x not in studies for x in required_study): missing_study.update(f"{cid}:{x}" for x in required_study if x not in studies); incomparable.add(cid)
        elif any(x not in modalities for x in required_modality): missing_modality.update(f"{cid}:{x}" for x in required_modality if x not in modalities); incomparable.add(cid)
        elif str(c.get("semantic_profile", "")) != str(request["semantic_profile"]) or str(c.get("comparability_digest", "")) != str(request["comparability_digest"]): incomparable.add(cid); uncertainty.add(f"{cid}:comparability-mismatch")
        elif not c.get("local") or not c.get("aggregate_only") or not c.get("policy_allowed") or str(c.get("replay_identity")) != str(request["replay_identity"]): blocked.add(cid)
        elif c.get("evidence_state") in {"proven", "supported"} and int(c.get("interpretation_score_milli", 0)) >= 600: qualified.add(cid)
        else: unresolved.add(cid)
    global_block = not all(request.get(k) for k in ("policy_allowed", "protected_closure", "signed_approval", "federation_allowed", "raw_data_local", "aggregate_only")) or bool(request.get("adversarial_event_order"))
    if global_block: blocked.update(ids); qualified.clear(); unresolved.clear(); incomparable.clear(); omissions.add("request:governance-or-adversarial-blocked")
    uncertainty.update(f"adversarial:{x}" for x in request.get("adversarial_event_order", ()))
    disposition = "blocked" if global_block else "unresolved" if unresolved or blocked or incomparable else "qualified"
    if disposition != "qualified": omissions.add("request:interpretation-closure-not-ready")
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "researcher": request["researcher"], "purpose": request["purpose"], "semantic_profile": request["semantic_profile"], "disposition": disposition, "candidate_order": ids, "qualified_order": sorted(qualified), "unresolved_order": sorted(unresolved), "blocked_order": sorted(blocked), "incomparable_order": sorted(incomparable), "missing_study_order": sorted(missing_study), "missing_modality_order": sorted(missing_modality), "omission_order": sorted(omissions), "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative), "replay_identity": request["replay_identity"], "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    digest = research_artifact_digest(payload); payload["interpretation_digest"] = digest; payload["artifact"] = {"artifact_id": f"interactive-interpretation-7:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": digest, "semantic_loss": payload["omission_order"], "provenance_digests": sorted(provenance), "boundary": PRECLINICAL_BOUNDARY}; payload["effect_receipts"] = [f"verify:ids-interpretation:{request['request_id']}" if disposition == "qualified" else "block:unsafe-release"]
    receipt = IdsInteractiveInterpretation7(payload); receipt.validate(); return receipt


def ids_interpretation_visualization_assurance_manifest() -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION, "owner_crate": "ids", "consumers": ["downstream AURORA crate maintainer", "interpretation reviewer", "federation operator"], "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "effects": ["verify:ids-interpretation", "block:unsafe-release"], "permissions": ["evaluate:capability-runs"], "autonomy_tier": "A1", "boundary": PRECLINICAL_BOUNDARY}

