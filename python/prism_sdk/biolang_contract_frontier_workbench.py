"""Python parity surface for ``AFA-biolang-P25-F20``.

The frontier workbench only compiles and compares signed BioLang capability
descriptors.  It never evaluates BioLang, moves raw observations, or turns
unknown evidence into a release decision.
"""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import re
from typing import Any, Mapping, Sequence

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-biolang-P25-F20"
CONTRACT_VERSION = "biolang-federated-continual-contract-frontier-workbench/1.0"
INPUT_SCHEMA = "BiolangContractInput4@1"
OUTPUT_SCHEMA = "BiolangCapabilityManifest5@1"
CONTENT_TYPE = "application/vnd.aurora.biolang-capability-manifest-5+json"


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _canonical(values: Sequence[str]) -> bool:
    return tuple(values) == tuple(sorted(set(values)))


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


@dataclass(frozen=True)
class BiolangCapabilityManifest:
    request_id: str
    federation_id: str
    operator_id: str
    requested_surface: str
    semantic_profile: str
    disposition: str
    contract_order: tuple[str, ...]
    ranked_order: tuple[str, ...]
    selected_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    missing_contract_order: tuple[str, ...]
    omission_order: tuple[str, ...]
    uncertainty_order: tuple[str, ...]
    negative_evidence_order: tuple[str, ...]
    adversarial_event_order: tuple[str, ...]
    replay_identity: str
    manifest_digest: str
    artifact: dict[str, Any]
    effect_receipts: tuple[str, ...]
    raw_data_local: bool = True
    aggregate_only: bool = True
    boundary: str = PRECLINICAL_BOUNDARY
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    contract_version: str = CONTRACT_VERSION
    feature_id: str = FEATURE_ID

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "contract_version": self.contract_version,
            "feature_id": self.feature_id,
            "request_id": self.request_id,
            "federation_id": self.federation_id,
            "operator_id": self.operator_id,
            "requested_surface": self.requested_surface,
            "semantic_profile": self.semantic_profile,
            "disposition": self.disposition,
            "contract_order": list(self.contract_order),
            "ranked_order": list(self.ranked_order),
            "selected_order": list(self.selected_order),
            "unknown_order": list(self.unknown_order),
            "blocked_order": list(self.blocked_order),
            "missing_contract_order": list(self.missing_contract_order),
            "omission_order": list(self.omission_order),
            "uncertainty_order": list(self.uncertainty_order),
            "negative_evidence_order": list(self.negative_evidence_order),
            "adversarial_event_order": list(self.adversarial_event_order),
            "replay_identity": self.replay_identity,
            "manifest_digest": self.manifest_digest,
            "artifact": self.artifact,
            "effect_receipts": list(self.effect_receipts),
            "raw_data_local": self.raw_data_local,
            "aggregate_only": self.aggregate_only,
            "boundary": self.boundary,
        }

    def validate(self) -> None:
        if (
            (self.schema_version, self.contract_version, self.feature_id)
            != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID)
            or self.boundary != PRECLINICAL_BOUNDARY
            or self.artifact.get("boundary") != PRECLINICAL_BOUNDARY
            or not self.raw_data_local
            or not self.aggregate_only
            or not all(v.strip() for v in (self.request_id, self.federation_id, self.operator_id, self.requested_surface, self.semantic_profile))
            or self.disposition not in {"qualified", "unresolved", "blocked"}
            or not self.contract_order
            or len(self.ranked_order) != len(self.contract_order)
            or not self.effect_receipts
        ):
            raise ResearchContractError("BioLang manifest identity, locality, ranking, or effects are incomplete")
        for values in (
            self.contract_order,
            self.selected_order,
            self.unknown_order,
            self.blocked_order,
            self.missing_contract_order,
            self.omission_order,
            self.uncertainty_order,
            self.negative_evidence_order,
            self.adversarial_event_order,
            self.effect_receipts,
        ):
            if not _canonical(values):
                raise ResearchContractError("BioLang manifest ordering is not canonical")
        ids = set(self.contract_order)
        parts = [*self.selected_order, *self.unknown_order, *self.blocked_order]
        if len(parts) != len(ids) or any(value not in ids for value in parts) or len(set(parts)) != len(parts) or set(self.ranked_order) != ids:
            raise ResearchContractError("BioLang contract states do not partition descriptors")
        if not all(_digest(value) for value in (self.replay_identity, self.manifest_digest, self.artifact.get("content_hash"))):
            raise ResearchContractError("BioLang manifest digest is invalid")
        if self.artifact.get("content_type") != CONTENT_TYPE:
            raise ResearchContractError("BioLang manifest artifact type is invalid")
        if self.disposition == "qualified" and self.effect_receipts != (f"view:contract-manifest:{self.request_id}",):
            raise ResearchContractError("qualified BioLang view effect is invalid")
        if self.disposition != "qualified" and self.effect_receipts != ("block:unsafe-release",):
            raise ResearchContractError("non-qualified BioLang manifest must block release")


def validate_contract_frontier(
    *,
    request_id: str,
    federation_id: str,
    operator_id: str,
    requested_surface: str,
    semantic_profile: str,
    required_contract_order: Sequence[str],
    descriptors: Sequence[Mapping[str, Any]],
    replay_identity: str,
    policy_allow: bool,
    protected_closure: bool,
    signed_approval: bool,
    federation_approved: bool,
    raw_data_local: bool,
    aggregate_only: bool,
    adversarial_events: Sequence[str] = (),
    boundary: str = PRECLINICAL_BOUNDARY,
) -> BiolangCapabilityManifest:
    if (
        not all(v.strip() for v in (request_id, federation_id, operator_id, requested_surface, semantic_profile))
        or not required_contract_order
        or not descriptors
        or not _canonical(required_contract_order)
        or not _canonical(adversarial_events)
        or not _digest(replay_identity)
        or boundary != PRECLINICAL_BOUNDARY
        or not raw_data_local
        or not aggregate_only
    ):
        raise ResearchContractError("BioLang input identity, contract closure, digest, locality, or boundary is invalid")
    rows = [dict(row) for row in descriptors]
    seen: set[str] = set()
    for row in rows:
        contract_id = str(row.get("contract_id", ""))
        if (
            not contract_id.strip()
            or contract_id in seen
            or not str(row.get("version", "")).strip()
            or not str(row.get("input_schema", "")).strip()
            or not str(row.get("output_schema", "")).strip()
            or not str(row.get("surface", "")).strip()
            or not str(row.get("semantic_profile", "")).strip()
            or not all(_digest(row.get(key)) for key in ("capability_digest", "provenance_digest", "compatibility_digest", "replay_identity"))
            or not _canonical(row.get("omissions", ()))
            or not _canonical(row.get("uncertainty", ()))
        ):
            raise ResearchContractError(f"descriptor {contract_id} is malformed or duplicated")
        seen.add(contract_id)
    rows.sort(key=lambda row: (str(row["contract_id"]), str(row["version"])))
    ranked = tuple(str(row["contract_id"]) for row in rows)
    order = tuple(sorted(ranked))
    required = set(required_contract_order)
    missing = tuple(sorted(required - set(order)))
    selected: set[str] = set()
    unknown: set[str] = set()
    blocked: set[str] = set()
    omission: set[str] = set()
    uncertainty: set[str] = set()
    negative: set[str] = set()
    for row in rows:
        contract_id = str(row["contract_id"])
        if row.get("negative_result"):
            negative.add(f"{contract_id}:negative-result")
        omission.update(f"{contract_id}:{value}" for value in row.get("omissions", ()))
        uncertainty.update(f"{contract_id}:{value}" for value in row.get("uncertainty", ()))
        state = str(row.get("evidence_state", ""))
        if state == "Contradicted":
            blocked.add(contract_id)
            negative.add(f"{contract_id}:contradicted-contract")
            continue
        if state in {"Unknown", "Speculative"}:
            unknown.add(contract_id)
            uncertainty.add(f"{contract_id}:evidence-unresolved")
            continue
        complete = (
            row["surface"] == requested_surface
            and row["semantic_profile"] == semantic_profile
            and bool(row.get("local_data"))
            and bool(row.get("permitted"))
            and not row.get("omissions")
            and not row.get("uncertainty")
            and all(_digest(row.get(key)) for key in ("capability_digest", "provenance_digest", "compatibility_digest"))
            and row["replay_identity"] == replay_identity
        )
        if complete and state in {"Proven", "Supported"}:
            selected.add(contract_id)
        else:
            unknown.add(contract_id)
            if row["surface"] != requested_surface:
                omission.add(f"{contract_id}:surface-mismatch")
            if row["semantic_profile"] != semantic_profile:
                omission.add(f"{contract_id}:semantic-profile-mismatch")
            if row["replay_identity"] != replay_identity:
                omission.add(f"{contract_id}:replay-mismatch")
            if not row.get("local_data") or not row.get("permitted"):
                blocked.add(contract_id)
                unknown.discard(contract_id)
                omission.add(f"{contract_id}:locality-or-permission-denied")
    omission.update(f"{value}:required-contract-missing" for value in missing)
    if not policy_allow:
        negative.add("request:policy-denied")
    if not protected_closure:
        uncertainty.add("request:protected-closure-incomplete")
    if not signed_approval or not federation_approved:
        uncertainty.add("request:institutional-approval-incomplete")
    negative.update(f"adversarial:{value}" for value in adversarial_events)
    global_block = not policy_allow or not protected_closure or not signed_approval or not federation_approved or not raw_data_local or not aggregate_only or bool(adversarial_events)
    disposition = "blocked" if global_block else "qualified" if not missing and selected and not unknown and not blocked else "unresolved"
    selected_order, unknown_order, blocked_order = tuple(sorted(selected)), tuple(sorted(unknown)), tuple(sorted(blocked))
    omission_order, uncertainty_order, negative_order = tuple(sorted(omission)), tuple(sorted(uncertainty)), tuple(sorted(negative))
    effects = (f"view:contract-manifest:{request_id}",) if disposition == "qualified" else ("block:unsafe-release",)
    payload = {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request_id,
        "federation_id": federation_id,
        "operator_id": operator_id,
        "requested_surface": requested_surface,
        "semantic_profile": semantic_profile,
        "disposition": disposition,
        "contract_order": list(order),
        "ranked_order": list(ranked),
        "selected_order": list(selected_order),
        "unknown_order": list(unknown_order),
        "blocked_order": list(blocked_order),
        "missing_contract_order": list(missing),
        "omission_order": list(omission_order),
        "uncertainty_order": list(uncertainty_order),
        "negative_evidence_order": list(negative_order),
        "adversarial_event_order": list(adversarial_events),
        "replay_identity": replay_identity,
        "effect_receipts": list(effects),
        "raw_data_local": raw_data_local,
        "aggregate_only": aggregate_only,
        "boundary": PRECLINICAL_BOUNDARY,
    }
    manifest_digest = _hash(payload)
    artifact = {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "artifact_id": f"biolang-capability-manifest:{request_id}",
        "content_type": CONTENT_TYPE,
        "content_hash": manifest_digest,
        "semantic_loss": [],
        "provenance": [],
        "boundary": PRECLINICAL_BOUNDARY,
    }
    result = BiolangCapabilityManifest(request_id, federation_id, operator_id, requested_surface, semantic_profile, disposition, order, ranked, selected_order, unknown_order, blocked_order, missing, omission_order, uncertainty_order, negative_order, tuple(adversarial_events), replay_identity, manifest_digest, artifact, effects, raw_data_local, aggregate_only)
    result.validate()
    return result


def biolang_contract_frontier_digest(result: BiolangCapabilityManifest) -> str:
    result.validate()
    return _hash(result.to_dict())


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "BiolangCapabilityManifest", "validate_contract_frontier", "biolang_contract_frontier_digest"]
