"""Provider-free reliance gating for autonomous outcomes.

Claim fusion, structured-response evaluation, and cross-domain alignment are deliberately
separate subsystems.  This module is the narrow contract that joins them at the point where an
application wants to rely on a completed result.  It binds explicit claim declarations to the
exact task, outcome, and output digests of one run, optionally requires the cross-domain response
gate to be complete, and emits only bounded metadata plus deterministic next actions.

The module does not extract claims from prose, decide scientific truth, settle an evaluator, or
authorize a provider/source/tool/effect.  Claim and evidence values remain caller-owned and are
only used transiently by the existing claim-integrity evaluator.
"""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import math
import re
from typing import Any, Mapping, Sequence

from .authoring import content_digest
from .autonomous_claim_integrity import (
    AutonomousClaimIntegrityAssessment,
    AutonomousClaimIntegrityClaim,
    AutonomousClaimIntegrityEvidence,
    AutonomousClaimIntegrityPolicy,
    assess_autonomous_claim_integrity,
)
from .autonomous_cross_domain_response import (
    AutonomousCrossDomainResponseAssessment,
    validate_autonomous_cross_domain_response_assessment,
)
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError


AUTONOMOUS_OUTCOME_INTEGRITY_SCHEMA = "bioprism-python-autonomous-outcome-integrity/0.1"
AUTONOMOUS_OUTCOME_INTEGRITY_RUN_SCHEMA = "bioprism-python-autonomous-outcome-integrity-run/0.1"
AUTONOMOUS_OUTCOME_INTEGRITY_BINDING_SCHEMA = "bioprism-python-autonomous-outcome-integrity-binding/0.1"
AUTONOMOUS_OUTCOME_INTEGRITY_STATUSES = ("ready", "review_required", "blocked", "ineligible")
AUTONOMOUS_OUTCOME_INTEGRITY_MODES = ("single_domain", "cross_domain")
AUTONOMOUS_OUTCOME_INTEGRITY_ROLES = ("run_output", "specialist_response", "synthesis_response")
MAX_AUTONOMOUS_OUTCOME_INTEGRITY_DOMAINS = len(AUTONOMOUS_DOMAIN_NAMES)
MAX_AUTONOMOUS_OUTCOME_INTEGRITY_CLAIM_BINDINGS = 512
MAX_AUTONOMOUS_OUTCOME_INTEGRITY_REASONS = 32
MAX_AUTONOMOUS_OUTCOME_INTEGRITY_ACTIONS = 32
MAX_AUTONOMOUS_OUTCOME_INTEGRITY_BYTES = 512_000
_DIGEST = re.compile(r"^[0-9a-f]{64}$")
_IDENTIFIER = re.compile(r"^[A-Za-z0-9_.:-]+$")
_RETENTION = "metadata_only;claims_evidence_responses_prompts_credentials_and_provider_values_not_retained"
_AUTHORITY = "provider_free_reliance_metadata_only;not_external_truth_or_execution_authority"


def _bounded_text(name: str, value: Any, maximum: int = 512) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise ArgumentError(f"{name} is outside its bounded text contract")
    if len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} exceeds its byte bound")
    return value.strip()


def _identifier(name: str, value: Any) -> str:
    text = _bounded_text(name, value, 256)
    if not _IDENTIFIER.fullmatch(text):
        raise ArgumentError(f"{name} is not a bounded identifier")
    return text


def _digest(name: str, value: Any, *, nullable: bool = False) -> str | None:
    if value is None and nullable:
        return None
    text = _bounded_text(name, value, 64)
    if not _DIGEST.fullmatch(text):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return text


def _safe_metadata(value: Any, name: str = "outcome integrity metadata", depth: int = 0) -> None:
    if depth > 16:
        raise ArgumentError(f"{name} is too deeply nested")
    if isinstance(value, Mapping):
        if len(value) > 512:
            raise ArgumentError(f"{name} contains too many fields")
        for key, child in value.items():
            if not isinstance(key, str) or not key.strip():
                raise ArgumentError(f"{name} contains an invalid field name")
            normalized = "".join(character for character in key.lower() if character.isalnum())
            if normalized == "secretmaterial" and child == "never_returned":
                continue
            if normalized in {
                "apikey", "authorization", "bearer", "credential", "credentials", "password", "secret",
                "secretkey", "token", "accesstoken", "refreshtoken", "privatekey", "clientsecret",
            } or any(marker in normalized for marker in ("token", "secret", "credential")):
                raise ArgumentError(f"{name}.{key} is credential-shaped metadata")
            _safe_metadata(child, f"{name}.{key}", depth + 1)
        return
    if isinstance(value, (list, tuple)):
        if len(value) > 512:
            raise ArgumentError(f"{name} contains too many entries")
        for index, child in enumerate(value):
            _safe_metadata(child, f"{name}[{index}]", depth + 1)
        return
    if isinstance(value, float) and not math.isfinite(value):
        raise ArgumentError(f"{name} contains a non-finite number")


def _domains(name: str, value: Any) -> tuple[str, ...]:
    if isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence):
        raise ArgumentError(f"{name} must be a domain sequence")
    if not 1 <= len(value) <= MAX_AUTONOMOUS_OUTCOME_INTEGRITY_DOMAINS:
        raise ArgumentError(f"{name} is outside its domain bound")
    result = tuple(_bounded_text(f"{name} entry", item, 64) for item in value)
    if any(domain not in AUTONOMOUS_DOMAIN_NAMES for domain in result):
        raise ArgumentError(f"{name} contains an unsupported domain")
    if len(set(result)) != len(result):
        raise ArgumentError(f"{name} contains duplicate domains")
    order = {domain: index for index, domain in enumerate(AUTONOMOUS_DOMAIN_NAMES)}
    return tuple(sorted(result, key=order.__getitem__))


def _response(value: Any) -> Any | None:
    candidate = value
    provider_loop = getattr(candidate, "provider_loop", None)
    if provider_loop is not None:
        final_response = getattr(provider_loop, "final_response", None)
        if final_response is not None:
            return final_response
    response = getattr(candidate, "response", None)
    if response is not None:
        return response
    brain_run = getattr(candidate, "brain_run", None)
    if brain_run is not None and brain_run is not candidate:
        return _response(brain_run)
    return None


def _response_payload(response: Any) -> dict[str, Any] | None:
    if response is None:
        return None
    text = getattr(response, "text", None)
    structured = getattr(response, "structured", None)
    if text is None and structured is None and isinstance(response, Mapping):
        text = response.get("text")
        structured = response.get("structured")
    if text is None and structured is None:
        return None
    return {"text": text if isinstance(text, str) else None, "structured": structured}


def _response_digest(value: Any, response_evaluation: Any = None) -> str | None:
    if isinstance(response_evaluation, Mapping) and isinstance(response_evaluation.get("response_digest"), str) and _DIGEST.fullmatch(response_evaluation["response_digest"]):
        return response_evaluation["response_digest"]
    payload = _response_payload(value)
    return None if payload is None else content_digest(payload)


@dataclass(frozen=True, slots=True)
class AutonomousOutcomeIntegrityRun:
    task_digest: str
    route_digest: str | None
    status: str
    mode: str
    domains: tuple[str, ...]
    output_digest: str | None
    response_digest: str | None
    outcome_digest: str
    response_assessment_digest: str | None = None
    response_assessment_status: str | None = None
    run_digest: str | None = None

    def __post_init__(self) -> None:
        task_digest = _digest("outcome integrity run task_digest", self.task_digest)
        route_digest = _digest("outcome integrity run route_digest", self.route_digest, nullable=True)
        status = _bounded_text("outcome integrity run status", self.status, 64)
        mode = _bounded_text("outcome integrity run mode", self.mode, 32)
        if mode not in AUTONOMOUS_OUTCOME_INTEGRITY_MODES:
            raise ArgumentError("outcome integrity run mode is unsupported")
        output_digest = _digest("outcome integrity run output_digest", self.output_digest, nullable=True)
        response_digest = _digest("outcome integrity run response_digest", self.response_digest, nullable=True)
        outcome_digest = _digest("outcome integrity run outcome_digest", self.outcome_digest)
        assessment_digest = _digest("outcome integrity response assessment digest", self.response_assessment_digest, nullable=True)
        assessment_status = None if self.response_assessment_status is None else _bounded_text("outcome integrity response assessment status", self.response_assessment_status, 64)
        normalized_domains = _domains("outcome integrity run domains", self.domains)
        descriptor = {
            "schema": AUTONOMOUS_OUTCOME_INTEGRITY_RUN_SCHEMA,
            "task_digest": task_digest,
            "route_digest": route_digest,
            "status": status,
            "mode": mode,
            "domains": list(normalized_domains),
            "output_digest": output_digest,
            "response_digest": response_digest,
            "outcome_digest": outcome_digest,
            "response_assessment_digest": assessment_digest,
            "response_assessment_status": assessment_status,
        }
        calculated = content_digest(descriptor)
        if self.run_digest is not None and self.run_digest != calculated:
            raise ArgumentError("outcome integrity run digest does not match its fields")
        object.__setattr__(self, "task_digest", task_digest)
        object.__setattr__(self, "route_digest", route_digest)
        object.__setattr__(self, "status", status)
        object.__setattr__(self, "mode", mode)
        object.__setattr__(self, "domains", normalized_domains)
        object.__setattr__(self, "output_digest", output_digest)
        object.__setattr__(self, "response_digest", response_digest)
        object.__setattr__(self, "outcome_digest", outcome_digest)
        object.__setattr__(self, "response_assessment_digest", assessment_digest)
        object.__setattr__(self, "response_assessment_status", assessment_status)
        object.__setattr__(self, "run_digest", calculated)

    def to_dict(self) -> dict[str, Any]:
        descriptor = {
            "schema": AUTONOMOUS_OUTCOME_INTEGRITY_RUN_SCHEMA,
            "task_digest": self.task_digest,
            "route_digest": self.route_digest,
            "status": self.status,
            "mode": self.mode,
            "domains": list(self.domains),
            "output_digest": self.output_digest,
            "response_digest": self.response_digest,
            "outcome_digest": self.outcome_digest,
            "response_assessment_digest": self.response_assessment_digest,
            "response_assessment_status": self.response_assessment_status,
        }
        return {**descriptor, "run_digest": self.run_digest}


@dataclass(frozen=True, slots=True)
class AutonomousOutcomeIntegrityClaimBinding:
    claim_id: str
    domain: str
    role: str
    output_digest: str
    response_digest: str | None
    binding_digest: str | None = None

    def __post_init__(self) -> None:
        claim_id = _identifier("outcome integrity binding claim_id", self.claim_id)
        domain = _bounded_text("outcome integrity binding domain", self.domain, 64)
        if domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("outcome integrity binding domain is unsupported")
        role = _bounded_text("outcome integrity binding role", self.role, 32)
        if role not in AUTONOMOUS_OUTCOME_INTEGRITY_ROLES:
            raise ArgumentError("outcome integrity binding role is unsupported")
        output_digest = _digest("outcome integrity binding output_digest", self.output_digest)
        response_digest = _digest("outcome integrity binding response_digest", self.response_digest, nullable=True)
        descriptor = {
            "schema": AUTONOMOUS_OUTCOME_INTEGRITY_BINDING_SCHEMA,
            "claim_id": claim_id,
            "domain": domain,
            "role": role,
            "output_digest": output_digest,
            "response_digest": response_digest,
        }
        calculated = content_digest(descriptor)
        if self.binding_digest is not None and self.binding_digest != calculated:
            raise ArgumentError("outcome integrity binding digest does not match its fields")
        object.__setattr__(self, "claim_id", claim_id)
        object.__setattr__(self, "domain", domain)
        object.__setattr__(self, "role", role)
        object.__setattr__(self, "output_digest", output_digest)
        object.__setattr__(self, "response_digest", response_digest)
        object.__setattr__(self, "binding_digest", calculated)

    def to_dict(self) -> dict[str, Any]:
        descriptor = {
            "schema": AUTONOMOUS_OUTCOME_INTEGRITY_BINDING_SCHEMA,
            "claim_id": self.claim_id,
            "domain": self.domain,
            "role": self.role,
            "output_digest": self.output_digest,
            "response_digest": self.response_digest,
        }
        return {**descriptor, "binding_digest": self.binding_digest}


@dataclass(frozen=True, slots=True)
class AutonomousOutcomeIntegrityAssessment:
    run: AutonomousOutcomeIntegrityRun
    claim_integrity_assessment_digest: str
    claim_integrity_status: str
    claim_count: int
    evidence_count: int
    claim_status_counts: Mapping[str, int]
    claim_action_ids: tuple[str, ...]
    claim_binding_digests: tuple[str, ...]
    response_assessment_digest: str | None
    response_assessment_status: str | None
    require_completed_run: bool
    require_response_assessment: bool
    require_synthesis: bool
    status: str
    gate_reasons: tuple[str, ...]
    next_actions: tuple[str, ...]
    assessment_digest: str | None = None

    def __post_init__(self) -> None:
        if not isinstance(self.run, AutonomousOutcomeIntegrityRun):
            raise ArgumentError("outcome integrity assessment requires a typed run")
        _digest("outcome integrity claim assessment digest", self.claim_integrity_assessment_digest)
        if self.claim_integrity_status not in {"ready", "partial", "blocked"}:
            raise ArgumentError("outcome integrity claim status is unsupported")
        for name, value in (("claim_count", self.claim_count), ("evidence_count", self.evidence_count)):
            if isinstance(value, bool) or not isinstance(value, int) or value < 0 or value > 512:
                raise ArgumentError(f"{name} is outside its bound")
        if not isinstance(self.claim_status_counts, Mapping) or not isinstance(self.claim_action_ids, Sequence) or isinstance(self.claim_action_ids, (str, bytes)):
            raise ArgumentError("outcome integrity claim metadata is malformed")
        _safe_metadata(self.claim_status_counts, "outcome integrity claim status counts")
        _safe_metadata(self.claim_action_ids, "outcome integrity claim action ids")
        if any(isinstance(value, bool) or not isinstance(value, int) or value < 0 or value > 512 for value in self.claim_status_counts.values()):
            raise ArgumentError("outcome integrity claim status counts are malformed")
        if any(_digest("outcome integrity claim action id", value) is None for value in self.claim_action_ids):
            raise ArgumentError("outcome integrity claim action ids are malformed")
        if self.status not in AUTONOMOUS_OUTCOME_INTEGRITY_STATUSES:
            raise ArgumentError("outcome integrity status is unsupported")
        for name, value in (("require_completed_run", self.require_completed_run), ("require_response_assessment", self.require_response_assessment), ("require_synthesis", self.require_synthesis)):
            if not isinstance(value, bool):
                raise ArgumentError(f"{name} must be boolean")
        reasons = tuple(_bounded_text("outcome integrity gate reason", value, 1_024) for value in self.gate_reasons)
        actions = tuple(_bounded_text("outcome integrity next action", value, 1_024) for value in self.next_actions)
        if len(set(reasons)) != len(reasons) or len(reasons) > MAX_AUTONOMOUS_OUTCOME_INTEGRITY_REASONS:
            raise ArgumentError("outcome integrity gate reasons are malformed")
        if len(set(actions)) != len(actions) or len(actions) > MAX_AUTONOMOUS_OUTCOME_INTEGRITY_ACTIONS:
            raise ArgumentError("outcome integrity next actions are malformed")
        _digest("outcome integrity response assessment digest", self.response_assessment_digest, nullable=True)
        if self.response_assessment_status is not None:
            _bounded_text("outcome integrity response assessment status", self.response_assessment_status, 64)
        normalized_counts = dict(sorted((str(key), int(value)) for key, value in self.claim_status_counts.items()))
        normalized_action_ids = tuple(sorted(str(value) for value in self.claim_action_ids))
        normalized_binding_digests = tuple(_digest("outcome integrity claim binding digest", value) for value in self.claim_binding_digests)
        if any(value is None for value in normalized_binding_digests):
            raise ArgumentError("outcome integrity claim binding digests are malformed")
        descriptor = {
            "schema": AUTONOMOUS_OUTCOME_INTEGRITY_SCHEMA,
            "run": self.run.to_dict(),
            "claim_integrity_assessment_digest": self.claim_integrity_assessment_digest,
            "claim_integrity_status": self.claim_integrity_status,
            "claim_count": self.claim_count,
            "evidence_count": self.evidence_count,
            "claim_status_counts": normalized_counts,
            "claim_action_ids": list(normalized_action_ids),
            "claim_binding_digests": list(normalized_binding_digests),
            "response_assessment_digest": self.response_assessment_digest,
            "response_assessment_status": self.response_assessment_status,
            "require_completed_run": self.require_completed_run,
            "require_response_assessment": self.require_response_assessment,
            "require_synthesis": self.require_synthesis,
            "status": self.status,
            "gate_reasons": list(reasons),
            "next_actions": list(actions),
            "retention": _RETENTION,
            "evaluator_authority": _AUTHORITY,
            "secret_material": "never_returned",
        }
        calculated = content_digest(descriptor)
        if self.assessment_digest is not None and self.assessment_digest != calculated:
            raise ArgumentError("outcome integrity assessment digest does not match its fields")
        object.__setattr__(self, "claim_status_counts", normalized_counts)
        object.__setattr__(self, "claim_action_ids", normalized_action_ids)
        object.__setattr__(self, "claim_binding_digests", tuple(str(value) for value in normalized_binding_digests))
        object.__setattr__(self, "gate_reasons", reasons)
        object.__setattr__(self, "next_actions", actions)
        object.__setattr__(self, "assessment_digest", calculated)

    def _descriptor(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_OUTCOME_INTEGRITY_SCHEMA,
            "run": self.run.to_dict(),
            "claim_integrity_assessment_digest": self.claim_integrity_assessment_digest,
            "claim_integrity_status": self.claim_integrity_status,
            "claim_count": self.claim_count,
            "evidence_count": self.evidence_count,
            "claim_status_counts": dict(self.claim_status_counts),
            "claim_action_ids": list(self.claim_action_ids),
            "claim_binding_digests": list(self.claim_binding_digests),
            "response_assessment_digest": self.response_assessment_digest,
            "response_assessment_status": self.response_assessment_status,
            "require_completed_run": self.require_completed_run,
            "require_response_assessment": self.require_response_assessment,
            "require_synthesis": self.require_synthesis,
            "status": self.status,
            "gate_reasons": list(self.gate_reasons),
            "next_actions": list(self.next_actions),
            "retention": _RETENTION,
            "evaluator_authority": _AUTHORITY,
            "secret_material": "never_returned",
        }

    def to_dict(self) -> dict[str, Any]:
        return {**self._descriptor(), "assessment_digest": self.assessment_digest}


def project_autonomous_outcome_integrity_run(
    result: Any,
    *,
    task_digest: str | None = None,
    domain: str | None = None,
) -> AutonomousOutcomeIntegrityRun:
    """Project a Python autonomous result to a value-free outcome identity.

    ``BrainRunResult`` carries a stable outcome digest but not always the route digest.  The
    high-level automatic and cross-domain result envelopes carry the route/blueprint identity;
    direct brain results may therefore supply the task digest and domain explicitly.
    """

    if result is None:
        raise ArgumentError("outcome integrity result is required")
    outer = result
    inner = getattr(result, "result", None)
    if inner is not None and not hasattr(result, "outcome_digest"):
        result = inner
    route = getattr(outer, "route", None)
    blueprint = getattr(outer, "blueprint", None)
    if blueprint is None:
        blueprint = getattr(result, "blueprint", None)
    resolved_task_digest = getattr(route, "task_digest", None) or getattr(blueprint, "task_digest", None) or task_digest
    if resolved_task_digest is None:
        plan = getattr(result, "plan", None)
        if isinstance(plan, Mapping):
            resolved_task_digest = plan.get("task_digest")
    resolved_task_digest = _digest("outcome integrity projected task_digest", resolved_task_digest)
    route_digest = getattr(route, "route_digest", None)
    if route_digest is None and isinstance(route, Mapping):
        route_digest = route.get("route_digest")
    route_digest = _digest("outcome integrity projected route_digest", route_digest, nullable=True)
    status = _bounded_text("outcome integrity projected status", getattr(outer, "status", getattr(result, "status", None)), 64)
    child_results = getattr(result, "child_results", None)
    synthesis_result = getattr(result, "synthesis_result", None)
    cross = isinstance(child_results, Sequence) and not isinstance(child_results, (str, bytes, bytearray)) or synthesis_result is not None
    mode = "cross_domain" if cross else "single_domain"
    if cross:
        child_blueprints = getattr(blueprint, "child_blueprints", ()) if blueprint is not None else ()
        resolved_domains = []
        for item in child_blueprints:
            profile = getattr(item, "profile", None)
            candidate = getattr(profile, "domain", None) or getattr(item, "domain", None)
            if isinstance(candidate, str):
                resolved_domains.append(candidate)
        resolved_domains.append("cross_domain")
        resolved_domains = tuple(dict.fromkeys(resolved_domains))
        source = synthesis_result
    else:
        profile = getattr(blueprint, "domain_pack", None)
        candidate = getattr(profile, "domain", None) if profile is not None else None
        resolved_domains = (domain or candidate or "coding",)
        source = result
    normalized_domains = _domains("outcome integrity projected domains", resolved_domains)
    response = _response(source)
    response_evaluation = getattr(source, "response_evaluation", None)
    output_digest = _response_digest(response)
    structural_digest = _response_digest(response, response_evaluation)
    raw_outcome_digest = getattr(source, "outcome_digest", None)
    if not isinstance(raw_outcome_digest, str) or not _DIGEST.fullmatch(raw_outcome_digest):
        child_digests = []
        if isinstance(child_results, Sequence) and not isinstance(child_results, (str, bytes, bytearray)):
            child_digests = [getattr(item, "outcome_digest", None) for item in child_results]
        raw_outcome_digest = content_digest({
            "schema": AUTONOMOUS_OUTCOME_INTEGRITY_RUN_SCHEMA,
            "status": status,
            "task_digest": resolved_task_digest,
            "route_digest": route_digest,
            "mode": mode,
            "child_outcome_digests": child_digests,
            "synthesis_outcome_digest": getattr(synthesis_result, "outcome_digest", None),
            "output_digest": output_digest,
            "response_digest": structural_digest,
        })
    response_assessment = getattr(outer, "response_assessment", None) or getattr(result, "response_assessment", None)
    response_assessment_digest = getattr(response_assessment, "assessment_digest", None)
    response_assessment_status = getattr(response_assessment, "status", None)
    return AutonomousOutcomeIntegrityRun(
        task_digest=resolved_task_digest,
        route_digest=route_digest,
        status=status,
        mode=mode,
        domains=normalized_domains,
        output_digest=output_digest,
        response_digest=structural_digest,
        outcome_digest=raw_outcome_digest,
        response_assessment_digest=response_assessment_digest,
        response_assessment_status=response_assessment_status,
    )


def _normalize_run(value: AutonomousOutcomeIntegrityRun | Mapping[str, Any]) -> AutonomousOutcomeIntegrityRun:
    if isinstance(value, AutonomousOutcomeIntegrityRun):
        return value
    if not isinstance(value, Mapping):
        raise ArgumentError("outcome integrity run must be typed or a mapping")
    return AutonomousOutcomeIntegrityRun(
        task_digest=value.get("task_digest"),
        route_digest=value.get("route_digest"),
        status=value.get("status"),
        mode=value.get("mode"),
        domains=tuple(value.get("domains", ())),
        output_digest=value.get("output_digest"),
        response_digest=value.get("response_digest"),
        outcome_digest=value.get("outcome_digest"),
        response_assessment_digest=value.get("response_assessment_digest"),
        response_assessment_status=value.get("response_assessment_status"),
        run_digest=value.get("run_digest"),
    )


def _normalize_binding(value: AutonomousOutcomeIntegrityClaimBinding | Mapping[str, Any], run: AutonomousOutcomeIntegrityRun) -> AutonomousOutcomeIntegrityClaimBinding:
    if isinstance(value, AutonomousOutcomeIntegrityClaimBinding):
        binding = value
    elif isinstance(value, Mapping):
        binding = AutonomousOutcomeIntegrityClaimBinding(
            claim_id=value.get("claim_id"),
            domain=value.get("domain"),
            role=value.get("role"),
            output_digest=value.get("output_digest"),
            response_digest=value.get("response_digest"),
            binding_digest=value.get("binding_digest"),
        )
    else:
        raise ArgumentError("outcome integrity claim binding must be typed or a mapping")
    if binding.output_digest != run.output_digest:
        raise ArgumentError("outcome integrity binding output_digest does not match the run output")
    if binding.response_digest != run.response_digest:
        raise ArgumentError("outcome integrity binding response_digest does not match the run response")
    return binding


def bind_autonomous_outcome_integrity_claims(
    run: AutonomousOutcomeIntegrityRun | Mapping[str, Any],
    bindings: Sequence[AutonomousOutcomeIntegrityClaimBinding | Mapping[str, Any]],
) -> tuple[AutonomousOutcomeIntegrityClaimBinding, ...]:
    """Normalize and digest claim bindings before building a reliance assessment."""

    normalized_run = _normalize_run(run)
    if isinstance(bindings, (str, bytes, bytearray)) or not isinstance(bindings, Sequence) or not 1 <= len(bindings) <= MAX_AUTONOMOUS_OUTCOME_INTEGRITY_CLAIM_BINDINGS:
        raise ArgumentError("outcome integrity claim bindings are outside their bound")
    normalized = tuple(_normalize_binding(value, normalized_run) for value in bindings)
    if len({value.claim_id for value in normalized}) != len(normalized):
        raise ArgumentError("outcome integrity claim bindings contain duplicate claim ids")
    return normalized


def _next_actions(status: str, reasons: Sequence[str], claim_assessment: AutonomousClaimIntegrityAssessment) -> tuple[str, ...]:
    actions: list[str] = []
    if "run_not_completed" in reasons:
        actions.append("inspect_incomplete_run")
    if "run_output_missing" in reasons:
        actions.append("obtain_reviewed_run_output")
    if "claim_bindings_incomplete" in reasons or "claim_binding_drift" in reasons:
        actions.append("rebind_claims_to_exact_run_output")
    if "claim_integrity_blocked" in reasons or claim_assessment.actions:
        actions.append("execute_reviewed_claim_integrity_actions")
    if "response_assessment_missing" in reasons or "response_alignment_incomplete" in reasons:
        actions.append("complete_cross_domain_response_review")
    if "synthesis_not_completed" in reasons:
        actions.append("complete_cross_domain_synthesis_review")
    if status == "review_required" and not actions:
        actions.append("obtain_caller_reliance_review")
    if status == "blocked" and not actions:
        actions.append("repair_blocked_outcome_contract")
    if status == "ineligible" and not actions:
        actions.append("wait_for_a_usable_autonomous_outcome")
    return tuple(dict.fromkeys(actions))[:MAX_AUTONOMOUS_OUTCOME_INTEGRITY_ACTIONS]


def assess_autonomous_outcome_integrity(
    *,
    run: AutonomousOutcomeIntegrityRun | Mapping[str, Any],
    claims: Sequence[AutonomousClaimIntegrityClaim | Mapping[str, Any]],
    evidence: Sequence[AutonomousClaimIntegrityEvidence | Mapping[str, Any]],
    claim_bindings: Sequence[AutonomousOutcomeIntegrityClaimBinding | Mapping[str, Any]],
    reference_time: str,
    policy: AutonomousClaimIntegrityPolicy | Mapping[str, Any] | None = None,
    response_assessment: AutonomousCrossDomainResponseAssessment | None = None,
    require_completed_run: bool = True,
    require_response_assessment: bool = False,
    require_synthesis: bool = False,
) -> AutonomousOutcomeIntegrityAssessment:
    normalized_run = _normalize_run(run)
    for name, value in (("require_completed_run", require_completed_run), ("require_response_assessment", require_response_assessment), ("require_synthesis", require_synthesis)):
        if not isinstance(value, bool):
            raise ArgumentError(f"{name} must be boolean")
    claim_assessment = assess_autonomous_claim_integrity(
        context_digest=normalized_run.task_digest,
        claims=claims,
        evidence=evidence,
        reference_time=reference_time,
        policy=policy,
    )
    if isinstance(claim_bindings, (str, bytes, bytearray)) or not isinstance(claim_bindings, Sequence) or len(claim_bindings) > MAX_AUTONOMOUS_OUTCOME_INTEGRITY_CLAIM_BINDINGS:
        raise ArgumentError("outcome integrity claim bindings are outside their bound")
    bindings = tuple(_normalize_binding(value, normalized_run) for value in claim_bindings)
    claim_ids = tuple(item.claim_id for item in claim_assessment.claims)
    binding_ids = tuple(item.claim_id for item in bindings)
    reasons: list[str] = []
    if len(set(binding_ids)) != len(binding_ids) or len(binding_ids) != len(claim_ids) or any(item not in claim_ids for item in binding_ids):
        reasons.append("claim_bindings_incomplete")
    if require_completed_run and normalized_run.status != "completed":
        reasons.append("run_not_completed")
    if normalized_run.output_digest is None:
        reasons.append("run_output_missing")
    if claim_assessment.status == "blocked":
        reasons.append("claim_integrity_blocked")
    elif claim_assessment.status != "ready":
        reasons.append("claim_integrity_requires_review")
    if response_assessment is not None:
        response_assessment = validate_autonomous_cross_domain_response_assessment(response_assessment)
        if response_assessment.context_digest != normalized_run.task_digest:
            raise ArgumentError("outcome integrity response assessment is bound to a different task")
        if normalized_run.response_assessment_digest is not None and response_assessment.assessment_digest != normalized_run.response_assessment_digest:
            reasons.append("response_assessment_digest_drift")
        if response_assessment.status not in {"completed", "ready_to_synthesize"}:
            reasons.append("response_alignment_incomplete")
        if require_synthesis and response_assessment.status != "completed":
            reasons.append("response_alignment_incomplete")
        if require_synthesis and not response_assessment.synthesis_domain_present:
            reasons.append("synthesis_not_completed")
    elif require_response_assessment:
        reasons.append("response_assessment_missing")
    if require_synthesis and response_assessment is None:
        reasons.append("synthesis_not_completed")
    if require_synthesis and normalized_run.mode != "cross_domain":
        reasons.append("synthesis_not_completed")
    unique_reasons = tuple(dict.fromkeys(reasons))[:MAX_AUTONOMOUS_OUTCOME_INTEGRITY_REASONS]
    if "run_output_missing" in unique_reasons:
        status = "ineligible"
    elif any(reason.endswith("blocked") or reason in {"claim_bindings_incomplete", "run_not_completed", "synthesis_not_completed"} for reason in unique_reasons):
        status = "blocked"
    elif unique_reasons:
        status = "review_required"
    else:
        status = "ready"
    counts: dict[str, int] = {}
    for item in claim_assessment.claims:
        counts[item.status] = counts.get(item.status, 0) + 1
    assessment = AutonomousOutcomeIntegrityAssessment(
        run=normalized_run,
        claim_integrity_assessment_digest=claim_assessment.assessment_digest,
        claim_integrity_status=claim_assessment.status,
        claim_count=len(claim_assessment.claims),
        evidence_count=len(claim_assessment.evidence),
        claim_status_counts=counts,
        claim_action_ids=tuple(item.action_id for item in claim_assessment.actions),
        claim_binding_digests=tuple(item.binding_digest for item in bindings),
        response_assessment_digest=(response_assessment.assessment_digest if response_assessment is not None else normalized_run.response_assessment_digest),
        response_assessment_status=(response_assessment.status if response_assessment is not None else normalized_run.response_assessment_status),
        require_completed_run=require_completed_run,
        require_response_assessment=require_response_assessment,
        require_synthesis=require_synthesis,
        status=status,
        gate_reasons=unique_reasons,
        next_actions=_next_actions(status, unique_reasons, claim_assessment),
    )
    if len(json.dumps(assessment.to_dict(), ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")) > MAX_AUTONOMOUS_OUTCOME_INTEGRITY_BYTES:
        raise ArgumentError("outcome integrity assessment exceeds its bound")
    return assessment


def validate_autonomous_outcome_integrity(value: AutonomousOutcomeIntegrityAssessment) -> AutonomousOutcomeIntegrityAssessment:
    if not isinstance(value, AutonomousOutcomeIntegrityAssessment):
        raise ArgumentError("outcome integrity validation requires a typed assessment")
    validate_autonomous_outcome_integrity_snapshot(value.to_dict())
    return value


def validate_autonomous_outcome_integrity_snapshot(value: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(value, Mapping) or value.get("schema") != AUTONOMOUS_OUTCOME_INTEGRITY_SCHEMA:
        raise ArgumentError("outcome integrity assessment schema is invalid")
    assessment_digest = value.get("assessment_digest")
    if not isinstance(assessment_digest, str):
        raise ArgumentError("outcome integrity assessment is missing its digest")
    descriptor = {key: item for key, item in value.items() if key != "assessment_digest"}
    if content_digest(descriptor) != assessment_digest:
        raise ArgumentError("outcome integrity assessment digest does not match its metadata")
    _safe_metadata(descriptor)
    run = _normalize_run(value.get("run"))
    if run.run_digest != value.get("run", {}).get("run_digest"):
        raise ArgumentError("outcome integrity assessment run digest is invalid")
    if value.get("status") == "ready" and value.get("gate_reasons") != []:
        raise ArgumentError("ready outcome integrity assessment cannot contain gate reasons")
    if value.get("status") == "ready" and value.get("next_actions") != []:
        raise ArgumentError("ready outcome integrity assessment cannot contain next actions")
    return dict(value)


__all__ = [
    "AUTONOMOUS_OUTCOME_INTEGRITY_SCHEMA",
    "AUTONOMOUS_OUTCOME_INTEGRITY_RUN_SCHEMA",
    "AUTONOMOUS_OUTCOME_INTEGRITY_BINDING_SCHEMA",
    "AUTONOMOUS_OUTCOME_INTEGRITY_STATUSES",
    "AUTONOMOUS_OUTCOME_INTEGRITY_MODES",
    "AUTONOMOUS_OUTCOME_INTEGRITY_ROLES",
    "AutonomousOutcomeIntegrityRun",
    "AutonomousOutcomeIntegrityClaimBinding",
    "AutonomousOutcomeIntegrityAssessment",
    "project_autonomous_outcome_integrity_run",
    "bind_autonomous_outcome_integrity_claims",
    "assess_autonomous_outcome_integrity",
    "validate_autonomous_outcome_integrity",
    "validate_autonomous_outcome_integrity_snapshot",
]
