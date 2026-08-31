"""Operator-facing review and dispatch-handoff projections for action admissions.

The controller is intentionally narrower than an execution engine. It gives an application a
bounded queue view, exact-record review operation, and downstream handoff. Authorization is
represented by a caller-supplied digest and must be verified by the deployment's identity system;
the controller never grants provider, source, tool, effect, learner, or credential authority.
"""

from __future__ import annotations

import re
from typing import Any, Mapping, Sequence

from .authoring import content_digest
from .autonomous_action_admission_persistence import (
    InMemoryAutonomousActionAdmissionLedger,
    validate_autonomous_action_admission_record,
)
from .autonomous_action_execution import AutonomousActionAdmission, admit_autonomous_action_plan
from .autonomous_action_plan import AutonomousActionPlan
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError


AUTONOMOUS_ACTION_REVIEW_QUEUE_SCHEMA = "bioprism-python-autonomous-action-review-queue/0.1"
AUTONOMOUS_ACTION_REVIEW_ROW_SCHEMA = "bioprism-python-autonomous-action-review-row/0.1"
AUTONOMOUS_ACTION_DISPATCH_HANDOFF_SCHEMA = "bioprism-python-autonomous-action-dispatch-handoff/0.1"
AUTONOMOUS_ACTION_REVIEW_RETENTION = "metadata_only;operator_review_projection_and_digests;task_prompt_provider_connector_credential_and_effect_values_not_retained"
AUTONOMOUS_ACTION_REVIEW_AUTHORITY = "caller_operator_projection_only;authorization_is_external_and_not_verified_by_sdk"
AUTONOMOUS_ACTION_REVIEW_EXECUTION = "review_control_only;does_not_authorize_provider_source_tool_effect_or_credentials"
AUTONOMOUS_ACTION_REVIEW_SECRET_MATERIAL = "never_returned"
AUTONOMOUS_ACTION_DISPATCH_DOWNSTREAM_GATES = (
    "credential_scope",
    "provider_or_source_approval",
    "tool_and_effect_authority",
    "evaluator_settlement",
)

_HANDOFF_KEYS = {
    "schema", "action_id", "record_digest", "plan_digest", "admission_digest", "plan", "admission",
    "selected_domains", "requested_domains", "cross_domain", "execution_path", "status", "downstream_gates",
    "authority", "retention", "execution", "secret_material", "handoff_digest",
}


def _fail(message: str) -> None:
    raise ArgumentError(f"autonomous action review controller {message}")


def _digest(name: str, value: Any) -> str:
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        _fail(f"{name} must be a lowercase SHA-256 digest")
    return value


def _domains(name: str, values: Sequence[str]) -> list[str]:
    if isinstance(values, (str, bytes, bytearray)) or not isinstance(values, Sequence) or not 1 <= len(values) <= len(AUTONOMOUS_DOMAIN_NAMES):
        _fail(f"{name} must contain one to twelve domains")
    normalized = [str(value) for value in values]
    if any(value not in AUTONOMOUS_DOMAIN_NAMES for value in normalized):
        _fail(f"{name} contains an unsupported domain")
    if len(set(normalized)) != len(normalized):
        _fail(f"{name} must contain unique domains")
    return normalized


def _action_id(value: Any) -> str:
    if not isinstance(value, str) or not 1 <= len(value) <= 256 or re.fullmatch(r"[A-Za-z0-9_.:+/-]+", value) is None:
        _fail("handoff action_id is invalid")
    return value


def _normalized(record: Mapping[str, Any]) -> tuple[dict[str, Any], AutonomousActionPlan, AutonomousActionAdmission]:
    normalized = validate_autonomous_action_admission_record(record)
    plan = AutonomousActionPlan.from_dict(normalized["plan"])
    admission = AutonomousActionAdmission.from_dict(normalized["admission"])
    return normalized, plan, admission


def _row(record: Mapping[str, Any]) -> dict[str, Any]:
    normalized, plan, admission = _normalized(record)
    return {
        "schema": AUTONOMOUS_ACTION_REVIEW_ROW_SCHEMA,
        "action_id": normalized["action_id"],
        "revision": normalized["revision"],
        "status": normalized["status"],
        "plan_digest": plan.plan_digest,
        "admission_digest": admission.admission_digest,
        "route_digest": plan.route_digest,
        "selected_domains": list(plan.selected_domains),
        "cross_domain": plan.cross_domain,
        "execution_path": admission.execution_path,
        "next_action": admission.next_action,
        "next_actions": list(admission.next_actions),
        "required_approvals": list(admission.required_approvals),
        "approved_approvals": list(admission.approved_approvals),
        "missing_approvals": list(admission.missing_approvals),
        "review_reasons": list(admission.review_reasons),
        "blocking_reasons": list(admission.blocking_reasons),
        "reviewer_digest": normalized["reviewer_digest"],
        "reason_digest": normalized["reason_digest"],
        "record_digest": normalized["record_digest"],
        "authority": AUTONOMOUS_ACTION_REVIEW_AUTHORITY,
        "retention": AUTONOMOUS_ACTION_REVIEW_RETENTION,
        "execution": AUTONOMOUS_ACTION_REVIEW_EXECUTION,
        "secret_material": AUTONOMOUS_ACTION_REVIEW_SECRET_MATERIAL,
    }


def _queue(rows: Sequence[Mapping[str, Any]]) -> dict[str, Any]:
    sorted_rows = sorted((dict(row) for row in rows), key=lambda row: row["action_id"])
    domain_counts = {domain: 0 for domain in AUTONOMOUS_DOMAIN_NAMES}
    for row in sorted_rows:
        for domain in row["selected_domains"]:
            domain_counts[domain] += 1
    body = {
        "schema": AUTONOMOUS_ACTION_REVIEW_QUEUE_SCHEMA,
        "rows": sorted_rows,
        "counts": {
            "total": len(sorted_rows),
            "pending_review": sum(row["status"] == "pending_review" for row in sorted_rows),
            "admitted": sum(row["status"] == "admitted" for row in sorted_rows),
            "blocked": sum(row["status"] == "blocked" for row in sorted_rows),
        },
        "domain_counts": domain_counts,
        "authority": AUTONOMOUS_ACTION_REVIEW_AUTHORITY,
        "retention": AUTONOMOUS_ACTION_REVIEW_RETENTION,
        "execution": AUTONOMOUS_ACTION_REVIEW_EXECUTION,
        "secret_material": AUTONOMOUS_ACTION_REVIEW_SECRET_MATERIAL,
    }
    return {**body, "queue_digest": content_digest(body)}


def validate_autonomous_action_dispatch_handoff(value: Mapping[str, Any]) -> dict[str, Any]:
    """Verify a handoff after it crosses persistence or process boundaries.

    This proves metadata continuity only. Reviewer authorization, credential readiness, provider
    availability, source truth, evaluator quality, and effect safety remain downstream gates.
    """

    if not isinstance(value, Mapping) or set(value) != _HANDOFF_KEYS:
        _fail("handoff contains unsupported or missing fields")
    if value["schema"] != AUTONOMOUS_ACTION_DISPATCH_HANDOFF_SCHEMA or value["authority"] != AUTONOMOUS_ACTION_REVIEW_AUTHORITY or value["retention"] != AUTONOMOUS_ACTION_REVIEW_RETENTION or value["execution"] != AUTONOMOUS_ACTION_REVIEW_EXECUTION or value["secret_material"] != AUTONOMOUS_ACTION_REVIEW_SECRET_MATERIAL:
        _fail("handoff markers are invalid")
    try:
        plan = AutonomousActionPlan.from_dict(value["plan"])
        admission = AutonomousActionAdmission.from_dict(value["admission"])
    except Exception as error:
        raise ArgumentError("autonomous action review controller handoff plan or admission is invalid") from error
    record_digest = _digest("handoff record_digest", value["record_digest"])
    plan_digest = _digest("handoff plan_digest", value["plan_digest"])
    admission_digest = _digest("handoff admission_digest", value["admission_digest"])
    supplied_digest = _digest("handoff handoff_digest", value["handoff_digest"])
    if plan_digest != plan.plan_digest or admission_digest != admission.admission_digest or plan.plan_digest != admission.plan_digest:
        _fail("handoff plan or admission digest is inconsistent")
    if admission.status != "admitted":
        _fail("handoff admission is not admitted")
    selected = _domains("handoff selected_domains", value["selected_domains"])
    requested = _domains("handoff requested_domains", value["requested_domains"])
    if selected != list(plan.selected_domains) or selected != list(admission.selected_domains):
        _fail("handoff selected domains are inconsistent")
    if any(domain not in selected for domain in requested):
        _fail("handoff requested domains exceed the selected domains")
    if not isinstance(value["cross_domain"], bool) or value["cross_domain"] != plan.cross_domain:
        _fail("handoff cross-domain posture is inconsistent")
    if not isinstance(value["execution_path"], str) or value["execution_path"] != admission.execution_path:
        _fail("handoff execution path is inconsistent")
    if value["status"] != "ready_for_downstream_gates":
        _fail("handoff status is invalid")
    if value["downstream_gates"] != list(AUTONOMOUS_ACTION_DISPATCH_DOWNSTREAM_GATES):
        _fail("handoff downstream gates are invalid")
    body = {
        "schema": AUTONOMOUS_ACTION_DISPATCH_HANDOFF_SCHEMA,
        "action_id": _action_id(value["action_id"]),
        "record_digest": record_digest,
        "plan_digest": plan_digest,
        "admission_digest": admission_digest,
        "plan": plan.to_dict(),
        "admission": admission.to_dict(),
        "selected_domains": selected,
        "requested_domains": requested,
        "cross_domain": value["cross_domain"],
        "execution_path": value["execution_path"],
        "status": "ready_for_downstream_gates",
        "downstream_gates": list(AUTONOMOUS_ACTION_DISPATCH_DOWNSTREAM_GATES),
        "authority": AUTONOMOUS_ACTION_REVIEW_AUTHORITY,
        "retention": AUTONOMOUS_ACTION_REVIEW_RETENTION,
        "execution": AUTONOMOUS_ACTION_REVIEW_EXECUTION,
        "secret_material": AUTONOMOUS_ACTION_REVIEW_SECRET_MATERIAL,
    }
    expected = content_digest(body)
    if supplied_digest != expected:
        _fail("handoff digest does not match metadata")
    return {**body, "handoff_digest": expected}


class AutonomousActionAdmissionController:
    """Bounded operator projection over an action-admission ledger."""

    def __init__(self, ledger: InMemoryAutonomousActionAdmissionLedger) -> None:
        if not isinstance(ledger, InMemoryAutonomousActionAdmissionLedger):
            _fail("requires a typed action admission ledger")
        self.ledger = ledger

    def queue(self) -> dict[str, Any]:
        return _queue([_row(record) for record in self.ledger.list()])

    def get(self, action_id: str) -> dict[str, Any] | None:
        record = self.ledger.get(action_id)
        return None if record is None else _row(record)

    def submit(
        self,
        action_id: str,
        plan: AutonomousActionPlan | Mapping[str, Any],
        *,
        approvals: Mapping[str, bool] | None = None,
        reviewed: bool = False,
        authorization_digest: str | None = None,
        reason: str | None = None,
    ) -> dict[str, Any]:
        """Submit a provider-free plan to the durable review queue.

        Ordinary submissions stay pending until :meth:`review` is called. A caller may submit
        an already-admitted plan only when it also supplies the external authorization digest;
        that digest is recorded as review identity, never treated as a provider credential.
        """

        parsed = plan if isinstance(plan, AutonomousActionPlan) else AutonomousActionPlan.from_dict(plan)
        admission = admit_autonomous_action_plan(parsed, approvals=approvals, reviewed=reviewed)
        authorization = None if authorization_digest is None else _digest("authorization_digest", authorization_digest)
        if admission.status == "admitted" and authorization is None:
            _fail("an admitted submission requires an authorization_digest")
        record = self.ledger.submit(
            parsed,
            admission,
            action_id=action_id,
            reviewer_digest=authorization,
            reason=reason,
        )
        return _row(record)

    def review(
        self,
        action_id: str,
        *,
        authorization_digest: str,
        approvals: Mapping[str, bool] | None = None,
        reviewed: bool = False,
        reason: str | None = None,
        expected_record_digest: str | None = None,
    ) -> dict[str, Any]:
        authorization = _digest("authorization_digest", authorization_digest)
        record = self.ledger.review(
            action_id,
            approvals=approvals,
            reviewed=reviewed,
            reviewer_digest=authorization,
            reason=reason,
            expected_record_digest=expected_record_digest,
        )
        return _row(record)

    def dispatch_handoff(self, action_id: str, requested_domains: Sequence[str] | None = None) -> dict[str, Any]:
        record = self.ledger.get(action_id)
        if record is None:
            _fail("cannot create a handoff for an unknown action")
        normalized, plan, admission = _normalized(record)
        if normalized["status"] != "admitted" or admission.status != "admitted":
            _fail("action admission is not ready for downstream gates")
        selected = list(plan.selected_domains)
        requested = _domains("requested_domains", selected if requested_domains is None else requested_domains)
        missing = sorted(set(requested) - set(selected))
        if missing:
            _fail("requested domains are outside the admitted plan: " + ", ".join(missing))
        body = {
            "schema": AUTONOMOUS_ACTION_DISPATCH_HANDOFF_SCHEMA,
            "action_id": normalized["action_id"],
            "record_digest": normalized["record_digest"],
            "plan_digest": plan.plan_digest,
            "admission_digest": admission.admission_digest,
            "plan": plan.to_dict(),
            "admission": admission.to_dict(),
            "selected_domains": selected,
            "requested_domains": requested,
            "cross_domain": plan.cross_domain,
            "execution_path": admission.execution_path,
            "status": "ready_for_downstream_gates",
            "downstream_gates": list(AUTONOMOUS_ACTION_DISPATCH_DOWNSTREAM_GATES),
            "authority": AUTONOMOUS_ACTION_REVIEW_AUTHORITY,
            "retention": AUTONOMOUS_ACTION_REVIEW_RETENTION,
            "execution": AUTONOMOUS_ACTION_REVIEW_EXECUTION,
            "secret_material": AUTONOMOUS_ACTION_REVIEW_SECRET_MATERIAL,
        }
        return validate_autonomous_action_dispatch_handoff({**body, "handoff_digest": content_digest(body)})


__all__ = [
    "AUTONOMOUS_ACTION_REVIEW_QUEUE_SCHEMA",
    "AUTONOMOUS_ACTION_REVIEW_ROW_SCHEMA",
    "AUTONOMOUS_ACTION_DISPATCH_HANDOFF_SCHEMA",
    "AUTONOMOUS_ACTION_REVIEW_RETENTION",
    "AUTONOMOUS_ACTION_REVIEW_AUTHORITY",
    "AUTONOMOUS_ACTION_REVIEW_EXECUTION",
    "AUTONOMOUS_ACTION_REVIEW_SECRET_MATERIAL",
    "AUTONOMOUS_ACTION_DISPATCH_DOWNSTREAM_GATES",
    "AutonomousActionAdmissionController",
    "validate_autonomous_action_dispatch_handoff",
]
