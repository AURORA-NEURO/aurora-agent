"""Python parity surface for conformance P14-F24.

This module validates and builds aggregate-only envelopes for continual interpretation and
visualization interoperability. It deliberately carries omissions and negative evidence instead
of turning incomplete closure into a confident scientific conclusion.
"""

from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)

FEATURE_ID = "AFA-conformance-P14-F24"
CONTRACT_VERSION = "conformance-federated-continual-interpretation-visualization-interoperability-gateway/1.0"
INPUT_SCHEMA = "InterpretationVisualizationRequest8@1"
OUTPUT_SCHEMA = "FederatedInterpretationVisualizationEnvelope10@1"
CONTENT_TYPE = "application/vnd.aurora.federated-interpretation-visualization-envelope-10+json"
_HEX = re.compile(r"^[0-9a-f]{64}$")


def _ordered(values: Sequence[str]) -> bool:
    return list(values) == sorted(set(values))


def _hash(value: Any) -> bool:
    return isinstance(value, str) and bool(_HEX.fullmatch(value))


@dataclass(frozen=True)
class FederatedInterpretationVisualizationEnvelope10:
    value: Mapping[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return dict(self.value)

    def validate(self) -> None:
        value = self.value
        artifact = value.get("artifact", {})
        if (
            value.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION
            or value.get("contract_version") != CONTRACT_VERSION
            or value.get("feature_id") != FEATURE_ID
            or value.get("boundary") != PRECLINICAL_BOUNDARY
            or artifact.get("boundary") != PRECLINICAL_BOUNDARY
            or artifact.get("content_type") != CONTENT_TYPE
            or value.get("raw_data_local") is not True
            or value.get("aggregate_only") is not True
            or not all(str(value.get(key, "")).strip() for key in ("request_id", "federation_id", "researcher", "purpose", "semantic_profile"))
            or int(value.get("checkpoint", 0)) <= 0
            or not value.get("candidate_order")
            or not value.get("ranked_order")
            or not value.get("peer_order")
            or not value.get("effect_receipts")
            or value.get("disposition") not in {"qualified", "unresolved", "blocked"}
        ):
            raise ResearchContractError("gateway identity, locality, orders, or release gate is incomplete")
        candidate_order = list(value["candidate_order"])
        ranked_order = list(value["ranked_order"])
        if len(set(candidate_order)) != len(candidate_order) or len(ranked_order) != len(candidate_order) or set(ranked_order) != set(candidate_order):
            raise ResearchContractError("candidate and ranked orders are not a deterministic permutation")
        for key in ("qualified_order", "unresolved_order", "blocked_order", "incomparable_order", "missing_study_order", "missing_modality_order", "missing_visualization_order", "peer_order", "qualified_peer_order", "missing_peer_order", "omission_order", "uncertainty_order", "contradiction_order", "negative_evidence_order", "support_witness", "quorum_witness", "effect_receipts"):
            if not _ordered(value.get(key, ())):
                raise ResearchContractError("gateway ordering is not canonical")
        states = list(value.get("qualified_order", ())) + list(value.get("unresolved_order", ())) + list(value.get("blocked_order", ())) + list(value.get("incomparable_order", ()))
        if len(states) != len(candidate_order) or set(states) != set(candidate_order) or len(set(states)) != len(states):
            raise ResearchContractError("candidate outcomes do not partition candidates")
        peer_order = list(value["peer_order"])
        peer_states = list(value.get("qualified_peer_order", ())) + list(value.get("missing_peer_order", ()))
        if len(set(peer_order)) != len(peer_order) or len(peer_states) != len(peer_order) or set(peer_states) != set(peer_order) or len(set(peer_states)) != len(peer_states):
            raise ResearchContractError("peer outcomes do not partition peers")
        for digest in (value.get("replay_identity"), value.get("interpretation_digest"), artifact.get("content_hash"), *artifact.get("provenance_digests", ())):
            if not _hash(digest):
                raise ResearchContractError("interpretation digest or artifact provenance is invalid")
        if artifact.get("content_hash") != value.get("interpretation_digest"):
            raise ResearchContractError("interpretation artifact metadata is inconsistent")
        if value["disposition"] == "qualified" and not any(str(effect).startswith("verify:interpretation-visualization:") for effect in value["effect_receipts"]):
            raise ResearchContractError("qualified gateway envelope lacks verification effect")
        if value["disposition"] != "qualified" and value["effect_receipts"] != ["block:unsafe-release"]:
            raise ResearchContractError("non-qualified gateway envelope must block release")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest(self.value)


def _validate_request(request: Mapping[str, Any]) -> None:
    if (
        request.get("schema_version") != INPUT_SCHEMA
        or not all(str(request.get(key, "")).strip() for key in ("request_id", "federation_id", "researcher", "purpose", "semantic_profile"))
        or not all(_ordered(request.get(key, ())) for key in ("required_study_order", "required_modality_order", "required_visualization_order", "adversarial_event_order"))
        or not all(request.get(key) for key in ("required_study_order", "required_modality_order", "required_visualization_order"))
        or int(request.get("checkpoint", 0)) <= 0
        or int(request.get("minimum_quorum", 0)) <= 0
        or not _hash(request.get("semantic_digest"))
        or not _hash(request.get("replay_identity"))
        or not isinstance(request.get("candidates"), Sequence)
        or not request.get("candidates")
        or not isinstance(request.get("peers"), Sequence)
        or not request.get("peers")
        or request.get("raw_data_local") is not True
        or request.get("aggregate_only") is not True
        or request.get("boundary") != PRECLINICAL_BOUNDARY
    ):
        raise ResearchContractError("gateway identity, requirements, quorum, digests, locality, or boundary is invalid")
    candidate_ids: set[str] = set()
    for candidate in request["candidates"]:
        cid = str(candidate.get("candidate_id", ""))
        if (
            not cid.strip() or cid in candidate_ids or not all(candidate.get(key) for key in ("study_order", "modality_order", "visualization_order"))
            or not all(_ordered(candidate.get(key, ())) for key in ("study_order", "modality_order", "visualization_order", "omission_order", "contradiction_order"))
            or not str(candidate.get("semantic_profile", "")).strip()
            or not 0 <= int(candidate.get("support_milli", -1)) <= 1000
            or not 0 <= int(candidate.get("uncertainty_milli", -1)) <= 1000
            or not all(_hash(candidate.get(key)) for key in ("semantic_digest", "artifact_digest", "provenance_digest", "replay_identity"))
        ):
            raise ResearchContractError("candidate identity, axes, scores, digests, or ordering is invalid")
        candidate_ids.add(cid)
    peer_ids: set[str] = set()
    for peer in request["peers"]:
        pid = str(peer.get("peer_id", ""))
        if (
            not pid.strip() or pid in peer_ids or not str(peer.get("origin", "")).strip() or not str(peer.get("semantic_profile", "")).strip()
            or int(peer.get("checkpoint", 0)) <= 0
            or not all(_hash(peer.get(key)) for key in ("semantic_digest", "artifact_digest", "provenance_digest", "replay_identity"))
        ):
            raise ResearchContractError("peer identity, checkpoint, digests, or origin is invalid")
        peer_ids.add(pid)


def assure_interpretation_visualization_gateway(request: Mapping[str, Any]) -> FederatedInterpretationVisualizationEnvelope10:
    _validate_request(request)
    rows = sorted(request["candidates"], key=lambda candidate: (-int(candidate["support_milli"]), int(candidate["uncertainty_milli"]), str(candidate["candidate_id"])))
    candidate_order = [str(candidate["candidate_id"]) for candidate in rows]
    peer_order = sorted(str(peer["peer_id"]) for peer in request["peers"])
    qualified_peers = sorted(
        str(peer["peer_id"])
        for peer in request["peers"]
        if peer.get("signed") is True and peer.get("aggregate_only") is True and peer.get("raw_data_local") is True and peer.get("policy_allowed") is True
        and str(peer.get("semantic_profile")) == str(request["semantic_profile"])
        and str(peer.get("semantic_digest")) == str(request["semantic_digest"])
        and str(peer.get("replay_identity")) == str(request["replay_identity"])
        and int(peer.get("checkpoint", 0)) >= int(request["checkpoint"])
    )
    missing_peers = sorted(set(peer_order) - set(qualified_peers))
    qualified: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set(); incomparable: set[str] = set()
    missing_study: set[str] = set(); missing_modality: set[str] = set(); missing_visualization: set[str] = set()
    omissions: set[str] = set(); uncertainty: set[str] = set(); contradiction: set[str] = set(); negative: set[str] = set(); support_witness: set[str] = set(); provenance: set[str] = set()
    for candidate in rows:
        cid = str(candidate["candidate_id"]); provenance.add(str(candidate["provenance_digest"]))
        omissions.update(f"{cid}:{item}" for item in candidate.get("omission_order", ())); uncertainty.update(f"{cid}:{item}" for item in candidate.get("uncertainty_order", ())); contradiction.update(f"{cid}:{item}" for item in candidate.get("contradiction_order", ()))
        if candidate.get("negative_result") or candidate.get("evidence_state") == "negative": negative.add(f"{cid}:negative-result")
        if not candidate.get("local") or not candidate.get("aggregate_only") or not candidate.get("policy_allowed") or str(candidate.get("replay_identity")) != str(request["replay_identity"]): blocked.add(cid)
        elif any(item not in candidate.get("study_order", ()) for item in request["required_study_order"]): missing_study.update(f"{cid}:{item}" for item in request["required_study_order"] if item not in candidate.get("study_order", ())); incomparable.add(cid)
        elif any(item not in candidate.get("modality_order", ()) for item in request["required_modality_order"]): missing_modality.update(f"{cid}:{item}" for item in request["required_modality_order"] if item not in candidate.get("modality_order", ())); incomparable.add(cid)
        elif any(item not in candidate.get("visualization_order", ()) for item in request["required_visualization_order"]): missing_visualization.update(f"{cid}:{item}" for item in request["required_visualization_order"] if item not in candidate.get("visualization_order", ())); incomparable.add(cid)
        elif not candidate.get("comparable") or str(candidate.get("semantic_profile")) != str(request["semantic_profile"]) or str(candidate.get("semantic_digest")) != str(request["semantic_digest"]): incomparable.add(cid); uncertainty.add(f"{cid}:semantic-comparability-mismatch")
        elif candidate.get("evidence_state") in {"contradicted", "negative"}: unresolved.add(cid); contradiction.add(f"{cid}:contradicted-or-negative")
        elif candidate.get("evidence_state") not in {"proven", "supported"} or int(candidate["support_milli"]) < 700 or int(candidate["uncertainty_milli"]) > 300: unresolved.add(cid); uncertainty.add(f"{cid}:support-or-uncertainty-threshold")
        else: qualified.add(cid); support_witness.add(f"{cid}:support={int(candidate['support_milli'])}milli")
    global_block = not all(request.get(key) is True for key in ("policy_allowed", "protected_closure", "signed_approval", "federation_allowed", "raw_data_local", "aggregate_only")) or bool(request.get("adversarial_event_order")) or len(qualified_peers) < int(request["minimum_quorum"])
    if global_block: blocked.update(candidate_order); qualified.clear(); unresolved.clear(); incomparable.clear(); omissions.add("request:governance-quorum-or-adversarial-blocked")
    uncertainty.update(f"adversarial:{item}" for item in request.get("adversarial_event_order", ()))
    qualified_order = sorted(qualified); unresolved_order = sorted(unresolved); blocked_order = sorted(blocked); incomparable_order = sorted(incomparable)
    disposition = "blocked" if global_block else "unresolved" if unresolved_order or blocked_order or incomparable_order else "qualified"
    omission_order = sorted(omissions | ({"request:interpretation-closure-not-ready"} if disposition != "qualified" else set()))
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "federation_id": request["federation_id"], "researcher": request["researcher"], "purpose": request["purpose"], "semantic_profile": request["semantic_profile"], "checkpoint": int(request["checkpoint"]), "disposition": disposition, "candidate_order": candidate_order, "ranked_order": candidate_order, "qualified_order": qualified_order, "unresolved_order": unresolved_order, "blocked_order": blocked_order, "incomparable_order": incomparable_order, "missing_study_order": sorted(missing_study), "missing_modality_order": sorted(missing_modality), "missing_visualization_order": sorted(missing_visualization), "peer_order": peer_order, "qualified_peer_order": qualified_peers, "missing_peer_order": missing_peers, "omission_order": omission_order, "uncertainty_order": sorted(uncertainty), "contradiction_order": sorted(contradiction), "negative_evidence_order": sorted(negative), "support_witness": sorted(support_witness), "quorum_witness": [f"qualified-peers={len(qualified_peers)}", f"required-quorum={int(request['minimum_quorum'])}"], "replay_identity": request["replay_identity"], "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    digest = research_artifact_digest(payload); payload["interpretation_digest"] = digest; payload["artifact"] = {"artifact_id": f"federated-interpretation-visualization-10:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": digest, "semantic_loss": omission_order, "provenance_digests": sorted(provenance), "boundary": PRECLINICAL_BOUNDARY}; payload["effect_receipts"] = [f"verify:interpretation-visualization:{request['request_id']}" if disposition == "qualified" else "block:unsafe-release"]
    envelope = FederatedInterpretationVisualizationEnvelope10(payload); envelope.validate(); return envelope


def interpretation_visualization_interoperability_gateway_manifest() -> dict[str, Any]:
    return {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "capability_id": FEATURE_ID, "version": CONTRACT_VERSION, "owner_crate": "conformance", "consumers": ["interpretation reviewer", "federation operator", "visualization workbench", "downstream conformance suite"], "behavior": "qualify continual federated interpretation and visualization envelopes using typed semantic, evidence, quorum, and policy gates", "value": "lets consortia compare reproducible aggregate interpretations without exporting raw experimental data", "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "effects": ["verify:interpretation-visualization", "emit:federation-envelope", "block:unsafe-release"], "permissions": ["evaluate:capability-runs", "federate:aggregate-research-artifacts"], "autonomy_tier": "A1", "boundary": PRECLINICAL_BOUNDARY}
