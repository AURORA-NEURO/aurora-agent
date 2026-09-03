"""Federated continual protocol simulation assurance for ``AFA-fiber-P10-F28``."""
from __future__ import annotations

from dataclasses import dataclass, asdict
import hashlib, json, re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-fiber-P10-F28"
CONTRACT_VERSION = "fiber-federated-continual-protocol-simulation-assurance/1.0"
INPUT_SCHEMA = "ProtocolDraft4@1"
OUTPUT_SCHEMA = "ProtocolSimulationReport7@1"

def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()
def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None
def _canonical(values: list[str] | tuple[str, ...]) -> bool:
    return list(values) == sorted(set(values))

@dataclass(frozen=True)
class FederatedProtocolSimulationReport:
    schema_version: str; contract_version: str; feature_id: str; request_id: str; federation_id: str; purpose: str; protocol_schema: str; semantic_profile: str; disposition: str
    required_step_order: tuple[str, ...]; observed_step_order: tuple[str, ...]; missing_step_order: tuple[str, ...]; violation_order: tuple[str, ...]; peer_order: tuple[str, ...]; qualified_peer_order: tuple[str, ...]; unresolved_peer_order: tuple[str, ...]; blocked_peer_order: tuple[str, ...]; adversarial_event_order: tuple[str, ...]; omissions: tuple[str, ...]; uncertainty: tuple[str, ...]; negative_evidence: tuple[str, ...]; protocol_digest: str; replay_identity: str; provenance_digest: str | None; peer_envelope_digest: str; verdict_digest: str; effect_receipts: tuple[str, ...]; artifact: Mapping[str, Any]; raw_data_local: bool; boundary: str
    def validate(self) -> None:
        if (self.schema_version, self.contract_version, self.feature_id, self.protocol_schema) != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID, INPUT_SCHEMA) or self.boundary != PRECLINICAL_BOUNDARY or self.raw_data_local is not True or not all(isinstance(value, str) and value.strip() for value in (self.request_id, self.federation_id, self.purpose, self.semantic_profile)) or not self.required_step_order or not self.peer_order or not self.effect_receipts:
            raise ResearchContractError("federated protocol identity, locality, steps, peers, or effects are incomplete")
        for values in (self.required_step_order, self.observed_step_order, self.missing_step_order, self.violation_order, self.peer_order, self.qualified_peer_order, self.unresolved_peer_order, self.blocked_peer_order, self.adversarial_event_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if not _canonical(list(values)): raise ResearchContractError("federated protocol ordering is not canonical")
        required = set(self.required_step_order); partition = list(self.observed_step_order) + list(self.missing_step_order)
        if set(partition) != required or len(partition) != len(set(partition)): raise ResearchContractError("protocol steps do not partition required steps")
        peers = set(self.peer_order); peer_partition = list(self.qualified_peer_order) + list(self.unresolved_peer_order) + list(self.blocked_peer_order)
        if set(peer_partition) != peers or len(peer_partition) != len(set(peer_partition)): raise ResearchContractError("peer states do not partition peers")
        for digest in (self.protocol_digest, self.replay_identity, self.peer_envelope_digest, self.verdict_digest, self.artifact.get("content_hash")):
            if not _digest(digest): raise ResearchContractError("federated protocol digest is invalid")
        if self.provenance_digest is not None and not _digest(self.provenance_digest): raise ResearchContractError("federated protocol provenance digest is invalid")
        if self.artifact.get("content_type") != "application/vnd.aurora.protocol-simulation-report+json": raise ResearchContractError("federated protocol artifact type is invalid")
        if any(not effect.startswith("verify:fiber-protocol-simulation:") and effect != "block:unsafe-release" for effect in self.effect_receipts): raise ResearchContractError("federated protocol effect is outside verification gate")
    def to_dict(self) -> dict[str, Any]:
        value = asdict(self)
        for key, item in value.items():
            if isinstance(item, tuple): value[key] = list(item)
        return value

def assure_federated_protocol(*, request: Mapping[str, Any]) -> FederatedProtocolSimulationReport:
    required = ("request_id", "federation_id", "purpose", "semantic_profile")
    if any(not str(request.get(field, "")).strip() for field in required) or request.get("protocol_schema") != INPUT_SCHEMA or not request.get("required_step_order") or not request.get("peer_institution_order") or not request.get("peers") or int(request.get("required_peer_quorum", 0)) <= 0 or int(request.get("required_peer_quorum", 0)) > len(request["peer_institution_order"]) or int(request.get("budget_units", 0)) <= 0 or int(request.get("budget_units", 0)) > int(request.get("max_budget_units", 0)) or request.get("raw_data_local") is not True or request.get("boundary") != PRECLINICAL_BOUNDARY or not _digest(request.get("protocol_digest")) or not _digest(request.get("replay_identity")):
        raise ResearchContractError("federated protocol identity, schema, steps, peers, bounds, replay, locality, or boundary is invalid")
    required_steps = [str(item) for item in request["required_step_order"]]; observed_steps = [str(item) for item in request.get("observed_step_order", [])]
    if not _canonical(required_steps) or not _canonical(observed_steps) or any(step not in set(required_steps) for step in observed_steps): raise ResearchContractError("protocol step orders are not canonical subsets")
    missing_steps = sorted(set(required_steps) - set(observed_steps)); peer_ids = [str(item) for item in request["peer_institution_order"]]; peers = sorted(request["peers"], key=lambda item: str(item.get("institution_id", "")))
    if not _canonical(peer_ids) or [str(item.get("institution_id", "")) for item in peers] != peer_ids: raise ResearchContractError("peer identities must cover the declared federation")
    qualified: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set()
    for peer in peers:
        pid = str(peer["institution_id"]); state = str(peer.get("evidence_state", "unknown")); hard = str(peer.get("semantic_profile", "")) != str(request["semantic_profile"]) or str(peer.get("replay_identity", "")) != str(request["replay_identity"]) or str(peer.get("protocol_digest", "")) != str(request["protocol_digest"]) or not _digest(peer.get("provenance_digest")) or peer.get("signed_approval") is not True or str(peer.get("permitted_artifact", "")) != "protocol-simulation" or state == "contradicted"
        if str(peer.get("semantic_profile", "")) != str(request["semantic_profile"]): omissions.add(f"peer:{pid}:semantic-profile-mismatch")
        if str(peer.get("replay_identity", "")) != str(request["replay_identity"]): omissions.add(f"peer:{pid}:replay-mismatch")
        if str(peer.get("protocol_digest", "")) != str(request["protocol_digest"]): omissions.add(f"peer:{pid}:protocol-mismatch")
        if not _digest(peer.get("provenance_digest")): omissions.add(f"peer:{pid}:provenance-missing")
        if peer.get("signed_approval") is not True: omissions.add(f"peer:{pid}:signed-approval-missing")
        if state in {"unknown", "speculative"}: uncertainty.add(f"peer:{pid}:evidence-state")
        for item in peer.get("omissions", []): omissions.add(f"peer:{pid}:{item}")
        for item in peer.get("uncertainty", []): uncertainty.add(f"peer:{pid}:{item}")
        negative.add(f"peer:{pid}:{'negative-result' if peer.get('negative_result') else 'negative-result-not-observed'}")
        if hard: blocked.add(pid)
        elif state in {"proven", "supported"}: qualified.add(pid)
        else: unresolved.add(pid)
    quorum = len(qualified) >= int(request["required_peer_quorum"])
    if not quorum: omissions.add(f"peer-quorum:{len(qualified)}/{request['required_peer_quorum']}"); uncertainty.add("federation:peer-quorum-incomplete")
    violations = {name for name, failed in (("policy", request.get("policy_allow") is not True),("protected-closure", request.get("protected_closure") is not True),("signed-approval", request.get("signed_approval") is not True),("federation-approval", request.get("federation_approved") is not True),("provenance", not _digest(request.get("provenance_digest")))) if failed}; violations.update(f"adversarial:{event}" for event in request.get("adversarial_events", []))
    if missing_steps: omissions.add(f"missing-steps:{','.join(missing_steps)}")
    if str(request.get("evidence_state", "unknown")) in {"unknown", "speculative"}: uncertainty.add("evidence-state-not-qualified")
    if str(request.get("evidence_state")) == "contradicted": violations.add("contradicted-evidence"); negative.add("local:contradicted-evidence")
    if not request.get("policy_allow"): omissions.add("workflow:policy-denied")
    if not request.get("protected_closure"): omissions.add("workflow:protected-closure-incomplete")
    if not request.get("signed_approval"): omissions.add("workflow:signed-approval-missing")
    if not request.get("federation_approved"): omissions.add("workflow:federation-approval-missing")
    omissions.update(f"workflow:adversarial:{event}" for event in request.get("adversarial_events", []))
    global_block = bool(violations or blocked or request.get("adversarial_events")); disposition = "blocked" if global_block else "unresolved" if missing_steps or not quorum or unresolved or uncertainty else "qualified"; qpeer = sorted(qualified); upeer = sorted(unresolved); bpeer = sorted(blocked)
    envelope = _hash({"federation_id":request["federation_id"],"purpose":request["purpose"],"protocol_digest":request["protocol_digest"],"peer_order":peer_ids,"qualified_peer_order":qpeer,"replay_identity":request["replay_identity"],"semantic_profile":request["semantic_profile"]}); payload = {"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request["request_id"],"federation_id":request["federation_id"],"purpose":request["purpose"],"protocol_schema":request["protocol_schema"],"semantic_profile":request["semantic_profile"],"required_step_order":required_steps,"observed_step_order":sorted(observed_steps),"missing_step_order":missing_steps,"violation_order":sorted(violations),"peer_order":peer_ids,"qualified_peer_order":qpeer,"unresolved_peer_order":upeer,"blocked_peer_order":bpeer,"replay_identity":request["replay_identity"],"peer_envelope_digest":envelope,"disposition":disposition,"boundary":PRECLINICAL_BOUNDARY}; verdict = _hash(payload); artifact={"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"artifact_id":f"fiber-protocol-simulation:{request['request_id']}","content_type":"application/vnd.aurora.protocol-simulation-report+json","content_hash":verdict,"semantic_loss":[],"provenance":[{"source_id":request["federation_id"],"relation":"federated-protocol-simulation-assurance","digest":verdict}],"boundary":PRECLINICAL_BOUNDARY}; checks=tuple(sorted({"schema-version","step-closure","peer-provenance","peer-semantic-profile","peer-replay-identity","peer-quorum","policy-boundary","negative-evidence-retention"})); report=FederatedProtocolSimulationReport(RESEARCH_CONTRACT_SCHEMA_VERSION,CONTRACT_VERSION,FEATURE_ID,str(request["request_id"]),str(request["federation_id"]),str(request["purpose"]),str(request["protocol_schema"]),str(request["semantic_profile"]),disposition,tuple(required_steps),tuple(sorted(observed_steps)),tuple(missing_steps),tuple(sorted(violations)),tuple(peer_ids),tuple(qpeer),tuple(upeer),tuple(bpeer),tuple(sorted(set(str(item) for item in request.get("adversarial_events", [])))),tuple(sorted(omissions)),tuple(sorted(uncertainty)),tuple(sorted(negative)),str(request["protocol_digest"]),str(request["replay_identity"]),str(request.get("provenance_digest")) if request.get("provenance_digest") else None,envelope,verdict,(f"verify:fiber-protocol-simulation:{request['request_id']}",) if disposition=="qualified" else ("block:unsafe-release",),artifact,True,PRECLINICAL_BOUNDARY); report.validate(); return report

__all__ = ["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","FederatedProtocolSimulationReport","assure_federated_protocol"]
