"""Python parity contract for policy- and grant-bound context release admission."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

CONTEXT_RELEASE_ADMISSION_FEATURE_ID = "AFA-brain-P03-F06"
CONTEXT_RELEASE_ADMISSION_CONTRACT_VERSION = "brain-context-release-admission/1.0"
CONTEXT_RELEASE_ACTION = "release:local-context"


@dataclass(frozen=True)
class BrainContextReleaseAdmissionReceipt:
    request_id: str
    disposition: str
    actor: str
    action: str
    context_digest: str
    omission_certificate_digest: str
    replay_identity: str
    policy_decision: str
    policy_reasons: tuple[str, ...]
    grant_scope: str
    grant_expiry: str
    remaining_units: float
    release_digest: str
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = CONTEXT_RELEASE_ADMISSION_FEATURE_ID
    contract_version: str = CONTEXT_RELEASE_ADMISSION_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != CONTEXT_RELEASE_ADMISSION_FEATURE_ID or self.contract_version != CONTEXT_RELEASE_ADMISSION_CONTRACT_VERSION:
            raise ResearchContractError("context release admission schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.request_id.strip() or not self.actor.strip() or self.action != CONTEXT_RELEASE_ACTION or not self.grant_scope.strip() or not self.grant_expiry.strip() or self.remaining_units < 0 or not self.policy_reasons or not self.effect_receipts or self.disposition not in {"admitted", "blocked", "approval_required", "unresolved"}:
            raise ResearchContractError("context release identity, policy, grant, budget, disposition, or effects are incomplete")
        for value in (self.context_digest, self.omission_certificate_digest, self.replay_identity, self.release_digest, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("context release digest is invalid")
        if any(not effect.startswith("release:local-context:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("context release effect is outside admission gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "disposition": self.disposition, "actor": self.actor, "action": self.action, "context_digest": self.context_digest, "omission_certificate_digest": self.omission_certificate_digest, "replay_identity": self.replay_identity, "policy_decision": self.policy_decision, "policy_reasons": list(self.policy_reasons), "grant_scope": self.grant_scope, "grant_expiry": self.grant_expiry, "remaining_units": self.remaining_units, "release_digest": self.release_digest, "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "boundary": self.boundary})


def admit_context_release(*, request_id: str, context_digest: str, omission_certificate_digest: str, replay_identity: str, policy: Mapping[str, Any], grant: Mapping[str, Any], requested_resource: str, requested_units: float) -> BrainContextReleaseAdmissionReceipt:
    if not request_id.strip() or not requested_resource.strip() or requested_units <= 0 or not all(isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) for value in (context_digest, omission_certificate_digest, replay_identity)):
        raise ResearchContractError("context release request identity, resource, budget, or digest is invalid")
    reasons = tuple(str(value) for value in policy.get("reasons", ()))
    artifacts = {str(value) for value in policy.get("evaluated_artifacts", ())}
    decision = str(policy.get("decision", "unresolved"))
    actor = str(grant.get("actor", "")); permitted = CONTEXT_RELEASE_ACTION in set(grant.get("permitted_actions", ())); budget = float(dict(grant.get("resource_budget", {})).get(requested_resource, 0.0)); remaining = max(0.0, budget - requested_units)
    identity_match = context_digest in artifacts and omission_certificate_digest in artifacts
    policy_ok = decision in {"allow", "local_only"} and "unresolved" not in reasons
    disposition = "blocked" if bool(grant.get("revoked", False)) or not permitted or budget < requested_units or not identity_match else ("approval_required" if not policy_ok or decision == "approval_required" else ("unresolved" if decision == "unresolved" else "admitted"))
    release_digest = research_artifact_digest({"feature_id": CONTEXT_RELEASE_ADMISSION_FEATURE_ID, "request_id": request_id, "context_digest": context_digest, "omission_certificate_digest": omission_certificate_digest, "replay_identity": replay_identity, "actor": actor, "action": CONTEXT_RELEASE_ACTION, "remaining_units": remaining, "disposition": disposition})
    effects = (f"release:local-context:{request_id}",) if disposition == "admitted" else ("block:unsafe-release",)
    artifact = {"content_hash": research_artifact_digest({"request_id": request_id, "release_digest": release_digest}), "media_type": "application/vnd.aurora.context-release-admission+json"}
    receipt = BrainContextReleaseAdmissionReceipt(request_id=request_id, disposition=disposition, actor=actor, action=CONTEXT_RELEASE_ACTION, context_digest=context_digest, omission_certificate_digest=omission_certificate_digest, replay_identity=replay_identity, policy_decision=decision, policy_reasons=reasons, grant_scope=str(grant.get("scope", "")), grant_expiry=str(grant.get("expires_at", "")), remaining_units=remaining, release_digest=release_digest, effect_receipts=effects, artifact=artifact)
    receipt.validate()
    return receipt
