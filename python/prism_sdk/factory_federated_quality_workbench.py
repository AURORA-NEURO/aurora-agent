"""Python parity for ``AFA-factory-P07-F20``.

The workbench evaluates caller-supplied quality attestations across a continually refreshed,
policy-separated federation. Raw measurements stay local; the result is a deterministic,
content-addressed quality verdict with explicit peer, modality, omission, and negative evidence.
"""
from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-factory-P07-F20"
CONTRACT_VERSION = "factory-federated-continual-quality-workbench/1.0"
INPUT_SCHEMA = "ResearchObject4@1"
OUTPUT_SCHEMA = "FactoryQualityVerdict5@1"
CONTENT_TYPE = "application/vnd.aurora.factory-quality-verdict-5+json"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _ordered(values: list[str]) -> bool:
    return values == sorted(set(values))


@dataclass(frozen=True)
class FactoryQualityVerdict5:
    value: dict[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return dict(self.value)

    def validate(self) -> None:
        v = self.value
        artifact = v.get("artifact", {})
        required = ("request_id", "federation_id", "study_id", "requester", "purpose", "semantic_profile")
        if not (
            v.get("schema_version") == RESEARCH_CONTRACT_SCHEMA_VERSION
            and v.get("contract_version") == CONTRACT_VERSION
            and v.get("feature_id") == FEATURE_ID
            and v.get("boundary") == PRECLINICAL_BOUNDARY
            and artifact.get("boundary") == PRECLINICAL_BOUNDARY
            and artifact.get("content_type") == CONTENT_TYPE
            and v.get("raw_data_local") is True and v.get("aggregate_only") is True
            and v.get("continual_epoch", 0) > 0 and v.get("peer_order")
            and v.get("effect_receipts") and v.get("observation_order") and v.get("modality_order")
            and all(isinstance(v.get(k), str) and v[k].strip() for k in required)
            and v.get("disposition") in {"qualified", "unresolved", "blocked"}
        ):
            raise ResearchContractError("factory quality identity, federation, locality, axes, or effects are incomplete")
        fields = ("observation_order", "passed_order", "failed_order", "unknown_order", "unmeasured_order", "blocked_order", "modality_order", "passed_modality_order", "missing_modality_order", "omission_order", "uncertainty_order", "negative_evidence_order", "peer_order", "qualified_peer_order", "missing_peer_order", "adversarial_event_order", "effect_receipts")
        if any(not _ordered(list(v.get(field, []))) for field in fields):
            raise ResearchContractError("factory quality ordering is not canonical")
        obs = set(v["observation_order"])
        parts = v["passed_order"] + v["failed_order"] + v["unknown_order"] + v["unmeasured_order"] + v["blocked_order"]
        if len(obs) != len(v["observation_order"]) or len(parts) != len(obs) or set(parts) != obs:
            raise ResearchContractError("factory quality observations do not partition")
        peers = set(v["peer_order"])
        peer_parts = v["qualified_peer_order"] + v["missing_peer_order"]
        if len(peers) != len(v["peer_order"]) or len(peer_parts) != len(peers) or set(peer_parts) != peers or v.get("quorum") != len(v["qualified_peer_order"]):
            raise ResearchContractError("factory quality peers do not partition")
        for d in (v.get("replay_identity"), v.get("report_digest"), artifact.get("content_hash")):
            if not _digest(d):
                raise ResearchContractError("factory quality digest is invalid")
        if artifact.get("content_hash") != v.get("report_digest"):
            raise ResearchContractError("factory quality artifact digest is inconsistent")
        effects = v["effect_receipts"]
        if any(not e.startswith("view:quality-workbench:") and e != "block:unsafe-release" for e in effects):
            raise ResearchContractError("factory quality effect is outside the governed view gate")
        if v["disposition"] == "qualified" and effects != [f"view:quality-workbench:{v['federation_id']}"]:
            raise ResearchContractError("qualified factory quality effect is invalid")
        if v["disposition"] != "qualified" and effects != ["block:unsafe-release"]:
            raise ResearchContractError("non-qualified factory quality verdict must block")

    def digest(self) -> str:
        self.validate()
        return _hash(self.value)


def factory_federated_quality_workbench_manifest() -> dict[str, Any]:
    return {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "capability_id": FEATURE_ID,
        "version": CONTRACT_VERSION,
        "owner_crate": "factory",
        "consumers": ["platform reliability engineer", "federation steward", "research administrator"],
        "behavior": "evaluates a federated continual multimodal quality envelope with typed thresholds and peer quorum",
        "value": "prevents failed, missing, contradictory, and unmeasured quality evidence from silently entering research workflows",
        "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA,
        "effects": ["execute_local_computation", "write_local_artifact"],
        "permissions": ["read:local-research-artifacts", "view:quality-workbench"],
        "autonomy_tier": "A1", "boundary": PRECLINICAL_BOUNDARY,
    }


def assure_factory_federated_quality_workbench(request: Mapping[str, Any]) -> FactoryQualityVerdict5:
    if not (
        request.get("schema_version") == RESEARCH_CONTRACT_SCHEMA_VERSION
        and all(isinstance(request.get(k), str) and request[k].strip() for k in ("request_id", "federation_id", "study_id", "requester", "purpose", "semantic_profile"))
        and request.get("continual_epoch", 0) > 0 and request.get("required_modalities") and request.get("required_peer_order")
        and _ordered(list(request["required_modalities"])) and _ordered(list(request["required_peer_order"]))
        and isinstance(request.get("minimum_peer_quorum"), int) and 0 < request["minimum_peer_quorum"] <= len(request["required_peer_order"])
        and isinstance(request.get("observations"), list) and request["observations"]
        and _digest(request.get("replay_identity")) and request.get("raw_data_local") is True and request.get("aggregate_only") is True
        and request.get("boundary") == PRECLINICAL_BOUNDARY
    ):
        raise ResearchContractError("factory quality request identity, federation, quorum, locality, or observations are invalid")
    peer_ids = set(); qualified_peers: set[str] = set()
    for peer in request.get("peers", []):
        pid = str(peer.get("peer_id", ""))
        if not pid or pid in peer_ids or pid not in request["required_peer_order"] or peer.get("semantic_profile") != request["semantic_profile"] or not _digest(peer.get("artifact_digest")) or not _digest(peer.get("replay_identity")):
            raise ResearchContractError("factory quality peer identity, profile, closure, or digest is invalid")
        peer_ids.add(pid)
        if all(peer.get(k) is True for k in ("signed", "permitted", "raw_data_local", "aggregate_only")) and peer.get("replay_identity") == request["replay_identity"] and peer.get("evidence_state") in {"proven", "supported"}:
            qualified_peers.add(pid)
    if peer_ids != set(request["required_peer_order"]):
        raise ResearchContractError("factory quality required peer closure is incomplete")
    rows = sorted((dict(row) for row in request["observations"]), key=lambda row: str(row.get("observation_id", "")))
    seen: set[str] = set(); passed: set[str] = set(); failed: set[str] = set(); unknown: set[str] = set(); unmeasured: set[str] = set(); blocked: set[str] = set(); modalities: set[str] = set(); passed_modalities: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set()
    for row in rows:
        oid = str(row.get("observation_id", "")); modality = str(row.get("modality", "")); seen.add(oid); modalities.add(modality)
        if row.get("negative_result"): negative.add(f"{oid}:negative-result")
        if row.get("study_id") != request["study_id"] or row.get("semantic_profile") != request["semantic_profile"] or row.get("replay_identity") != request["replay_identity"] or not all(row.get(k) is True for k in ("signed", "permitted", "raw_data_local", "aggregate_only")):
            unknown.add(oid); uncertainty.add(f"{oid}:scope-or-authorization")
        elif row.get("evidence_state") == "contradicted":
            blocked.add(oid); negative.add(f"{oid}:contradicted")
        elif row.get("evidence_state") in {"unknown", "unmeasured"}:
            (unknown if row.get("evidence_state") == "unknown" else unmeasured).add(oid); uncertainty.add(f"{oid}:{row.get('evidence_state')}")
        elif int(row.get("value_milli", 0)) >= int(row.get("threshold_milli", 0)):
            passed.add(oid); passed_modalities.add(modality)
        else:
            failed.add(oid); omissions.add(f"{oid}:threshold-failed")
    missing = set(request["required_modalities"]) - passed_modalities
    if missing: omissions.add("modality:required-closure-incomplete")
    if request.get("adversarial_event_order"): negative.update(f"adversarial:{e}" for e in request["adversarial_event_order"])
    global_block = not all(request.get(k) is True for k in ("policy_allow", "protected_closure", "signed_approval", "raw_data_local", "aggregate_only")) or bool(request.get("adversarial_event_order")) or len(qualified_peers) < request["minimum_peer_quorum"]
    if global_block: blocked.update(seen); passed.clear(); failed.clear(); unknown.clear(); unmeasured.clear(); omissions.add("request:quality-release-gate-blocked")
    disposition = "blocked" if global_block or blocked else "unresolved" if failed or unknown or unmeasured or missing else "qualified"
    peer_order = sorted(peer_ids); qualified_peer_order = sorted(qualified_peers); missing_peer_order = sorted(peer_ids - qualified_peers)
    effects = [f"view:quality-workbench:{request['federation_id']}"] if disposition == "qualified" else ["block:unsafe-release"]
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "federation_id": request["federation_id"], "study_id": request["study_id"], "requester": request["requester"], "purpose": request["purpose"], "semantic_profile": request["semantic_profile"], "checkpoint": int(request.get("checkpoint", 1)), "continual_epoch": request["continual_epoch"], "disposition": disposition, "observation_order": sorted(seen), "passed_order": sorted(passed), "failed_order": sorted(failed), "unknown_order": sorted(unknown), "unmeasured_order": sorted(unmeasured), "blocked_order": sorted(blocked), "modality_order": sorted(modalities | set(request["required_modalities"])), "passed_modality_order": sorted(passed_modalities), "missing_modality_order": sorted(missing), "omission_order": sorted(omissions), "uncertainty_order": sorted(uncertainty), "negative_evidence_order": sorted(negative), "peer_order": peer_order, "qualified_peer_order": qualified_peer_order, "missing_peer_order": missing_peer_order, "adversarial_event_order": sorted(request.get("adversarial_event_order", [])), "quorum": len(qualified_peer_order), "minimum_peer_quorum": request["minimum_peer_quorum"], "pass_fraction_milli": (len(passed) * 1000) // max(len(rows), 1), "replay_identity": request["replay_identity"], "effect_receipts": effects, "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    digest = _hash(payload); payload["report_digest"] = digest; payload["artifact"] = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"factory-quality-verdict-5:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": digest, "semantic_loss": [], "provenance_digests": sorted({str(row.get("provenance_digest")) for row in rows if _digest(row.get("provenance_digest"))}), "boundary": PRECLINICAL_BOUNDARY}
    receipt = FactoryQualityVerdict5(payload); receipt.validate(); return receipt


def factoryFederatedQualityWorkbenchDigest(receipt: FactoryQualityVerdict5) -> str:
    return receipt.digest()


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "CONTENT_TYPE", "FactoryQualityVerdict5", "factory_federated_quality_workbench_manifest", "assure_factory_federated_quality_workbench", "factoryFederatedQualityWorkbenchDigest"]
