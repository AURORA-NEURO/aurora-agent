"""Catalogue-backed evidence composition for the Python autonomous agent.

The ordinary evidence runtime accepts one caller-owned acquirer.  The domain catalogue adds
reviewed source profiles, route selection, and digest-bound normalizers, but it is intentionally
provider-neutral.  This module closes the application-facing gap: it prepares every evidence
requirement in a selected domain set, reconciles the routes with bounded parallelism, and feeds a
metadata-only evidence context into the existing routing, prompt, provider, memory, and learning
path.

Source dispatch, evidence settlement, and provider invocation remain independent approvals. Raw
source and normalized values are available only through the transient result and explicit prompt
builder; ``to_dict`` never contains them.
"""

from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
import json
import math
from typing import Any, Callable, Mapping, Sequence

from .authoring import content_digest
from .autonomy import AUTONOMOUS_DOMAINS
from .autonomous_domain_evidence_catalogue import (
    AutonomousDomainEvidenceCatalogueReconciliation,
    AutonomousDomainEvidenceSourceCatalogue,
)
from .autonomous_evidence import AutonomousEvidencePlan, AutonomousEvidenceRequirement
from .autonomous_evidence_reconciliation import AutonomousEvidenceReconciliationResult
from .brain import BrainRunError
from .errors import ArgumentError


AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_RUN_SCHEMA = "bioprism-python-autonomous-domain-evidence-brain-run/0.1"
AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_CONTEXT_SCHEMA = "bioprism-python-autonomous-domain-evidence-brain-context/0.1"
AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_STATUSES = (
    "evidence_review_required",
    "evidence_blocked",
    "evidence_failed",
    "evidence_incomplete",
    "provider_review_required",
    "provider_failed",
    "completed",
)
MAX_AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_REQUIREMENTS = 256
MAX_AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_PARALLEL_REQUIREMENTS = 8
MAX_AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_CONTEXT_BYTES = 64_000
MAX_AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_RESULT_BYTES = 512_000
_RETENTION = "metadata_only;source_values_prompt_values_and_provider_response_caller_owned"
_SECRET_MARKERS = {
    "apikey", "authorization", "bearer", "credential", "credentials", "password", "secret",
    "secretkey", "token", "accesstoken", "refreshtoken", "privatekey", "clientsecret", "gsk", "sk",
}


def _bounded_task(value: Any) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > 32_000:
        raise ArgumentError("domain evidence brain task must be bounded non-empty text")
    return value.strip()


def _bounded_domains(value: Any) -> tuple[str, ...]:
    if value is None:
        selected = tuple(AUTONOMOUS_DOMAINS)
    elif isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence):
        raise ArgumentError("domain evidence brain domains must be a sequence")
    else:
        selected = tuple(value)
    if not 1 <= len(selected) <= len(AUTONOMOUS_DOMAINS):
        raise ArgumentError("domain evidence brain domains are outside their bound")
    result: list[str] = []
    for index, domain in enumerate(selected):
        if not isinstance(domain, str) or domain not in AUTONOMOUS_DOMAINS:
            raise ArgumentError(f"domain evidence brain domain {index} is unsupported")
        if domain in result:
            raise ArgumentError("domain evidence brain domains contain duplicates")
        result.append(domain)
    return tuple(result)


def _mapping_options(name: str, value: Any) -> dict[str, Any]:
    if value is None:
        return {}
    if not isinstance(value, Mapping):
        raise ArgumentError(f"{name} must be a mapping")
    return dict(value)


def _assert_safe_transient(value: Any, name: str, depth: int = 0) -> None:
    if depth > 32:
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
            if normalized in _SECRET_MARKERS or any(marker in normalized for marker in ("token", "secret", "credential")):
                raise ArgumentError(f"{name}.{key} is credential-shaped transient data")
            _assert_safe_transient(child, f"{name}.{key}", depth + 1)
        return
    if isinstance(value, (list, tuple)):
        if len(value) > 512:
            raise ArgumentError(f"{name} contains too many entries")
        for index, child in enumerate(value):
            _assert_safe_transient(child, f"{name}[{index}]", depth + 1)
        return
    if isinstance(value, float) and not math.isfinite(value):
        raise ArgumentError(f"{name} contains a non-finite number")


def _safe_context(value: Any, name: str = "domain evidence brain prompt context") -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise ArgumentError(f"{name} must be a mapping")
    _assert_safe_transient(value, name)
    try:
        encoded = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False)
    except (TypeError, ValueError) as error:
        raise ArgumentError(f"{name} must be JSON-safe") from error
    if len(encoded.encode("utf-8")) > MAX_AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_CONTEXT_BYTES:
        raise ArgumentError(f"{name} exceeds its byte bound")
    return dict(value)


def _safe_task_digest(task: str) -> str:
    return content_digest({"task": task})


@dataclass(frozen=True, slots=True)
class AutonomousDomainEvidenceBrainPreparation:
    requirement_id: str
    domain: str
    prepared: AutonomousDomainEvidenceCatalogueReconciliation
    result: AutonomousEvidenceReconciliationResult | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "requirement_id": self.requirement_id,
            "domain": self.domain,
            "profile": dict(self.prepared.profile),
            "plan": self.prepared.plan.to_dict(),
            "routes": [dict(route) for route in self.prepared.routes],
            "normalizer_registry_digest": self.prepared.normalizer_registry_digest,
            "result_digest": None if self.result is None else self.result.result_digest,
        }


@dataclass(frozen=True, slots=True)
class AutonomousDomainEvidenceBrainPromptProjection:
    plan: AutonomousEvidencePlan
    prepared: tuple[AutonomousDomainEvidenceBrainPreparation, ...]
    values: Mapping[str, Mapping[str, Any]]
    normalized_values: Mapping[str, Mapping[str, Any]]


@dataclass(frozen=True, slots=True)
class AutonomousDomainEvidenceBrainPreflight:
    plan: AutonomousEvidencePlan
    prepared: tuple[AutonomousDomainEvidenceBrainPreparation, ...]
    prompt_context: Mapping[str, Any]


@dataclass(frozen=True, slots=True)
class AutonomousDomainEvidenceBrainRunResult:
    status: str
    task_digest: str
    execution_plan_digest: str
    evidence_plan: AutonomousEvidencePlan
    prepared: tuple[AutonomousDomainEvidenceBrainPreparation, ...]
    prompt_context: Mapping[str, Any]
    execution: Any | None
    execution_status: str | None
    route_digest: str | None
    execution_digest: str | None
    catalogue_digest: str
    normalizer_registry_digest: str
    result_digest: str

    def to_dict(self) -> dict[str, Any]:
        descriptor = {
            "schema": AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_RUN_SCHEMA,
            "status": self.status,
            "task_digest": self.task_digest,
            "execution_plan_digest": self.execution_plan_digest,
            "evidence_plan_digest": self.evidence_plan.plan_digest,
            "catalogue_digest": self.catalogue_digest,
            "normalizer_registry_digest": self.normalizer_registry_digest,
            "prepared": [item.to_dict() for item in self.prepared],
            "reconciliations": [None if item.result is None else item.result.to_dict() for item in self.prepared],
            "prompt_context_digest": None if not self.prompt_context else content_digest(self.prompt_context),
            "execution_status": self.execution_status,
            "route_digest": self.route_digest,
            "execution_digest": self.execution_digest,
            "retention": _RETENTION,
            "secret_material": "never_returned",
        }
        return {**descriptor, "result_digest": self.result_digest}


def _prepared_projection(prepared: Sequence[AutonomousDomainEvidenceBrainPreparation]) -> list[dict[str, Any]]:
    return [item.to_dict() for item in prepared]


def _execution_metadata(agent: Any, execution: Any) -> tuple[str | None, str | None, str | None]:
    if execution is None:
        return None, None, None
    status = getattr(execution, "execution_status", getattr(execution, "status", None))
    normalized_status = status if isinstance(status, str) else None
    route_digest = None
    route = getattr(execution, "route", None)
    candidate = getattr(route, "route_digest", None)
    if isinstance(candidate, str) and len(candidate) == 64:
        route_digest = candidate
    try:
        metadata = agent._trace_execution_metadata(execution)
    except Exception:
        metadata = {"status": normalized_status, "result_type": execution.__class__.__name__}
    if isinstance(metadata, Mapping):
        candidate = metadata.get("route_digest")
        if isinstance(candidate, str) and len(candidate) == 64:
            route_digest = candidate
        execution_digest = content_digest({key: value for key, value in metadata.items() if key not in {"response", "prompt", "task", "values"}})
    else:
        execution_digest = content_digest({"status": normalized_status, "result_type": execution.__class__.__name__})
    return normalized_status, route_digest, execution_digest


def _status_for_evidence(prepared: Sequence[AutonomousDomainEvidenceBrainPreparation]) -> str:
    statuses = [item.result.status for item in prepared if item.result is not None]
    return "evidence_failed" if statuses and all(status == "failed" for status in statuses) else "evidence_incomplete"


def _default_prompt_context(plan: AutonomousEvidencePlan, prepared: Sequence[AutonomousDomainEvidenceBrainPreparation]) -> dict[str, Any]:
    context = {
        "catalogue_reviewed_evidence": {
            "schema": AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_CONTEXT_SCHEMA,
            "evidence_plan_digest": plan.plan_digest,
            "reconciliations": [
                {
                    "requirement_id": item.requirement_id,
                    "domain": item.domain,
                    "profile_id": item.prepared.profile.get("profile_id"),
                    "profile_digest": item.prepared.profile.get("profile_digest"),
                    "normalizer_id": item.prepared.profile.get("normalizer_id"),
                    "normalizer_version": item.prepared.profile.get("normalizer_version"),
                    "reconciliation_plan_digest": item.prepared.plan.plan_digest,
                    "result": None if item.result is None else item.result.to_dict(),
                }
                for item in prepared
            ],
            "retention": _RETENTION,
            "secret_material": "never_returned",
        }
    }
    encoded = json.dumps(context, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False)
    if len(encoded.encode("utf-8")) > MAX_AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_CONTEXT_BYTES:
        raise ArgumentError("default domain evidence brain context exceeds its byte bound")
    return context


def _build_result(
    *,
    status: str,
    task_digest: str,
    execution_plan_digest: str,
    plan: AutonomousEvidencePlan,
    prepared: Sequence[AutonomousDomainEvidenceBrainPreparation],
    prompt_context: Mapping[str, Any],
    execution: Any | None,
    execution_status: str | None,
    route_digest: str | None,
    execution_digest: str | None,
    catalogue_digest: str,
    normalizer_registry_digest: str,
) -> AutonomousDomainEvidenceBrainRunResult:
    descriptor = {
        "schema": AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_RUN_SCHEMA,
        "status": status,
        "task_digest": task_digest,
        "execution_plan_digest": execution_plan_digest,
        "evidence_plan_digest": plan.plan_digest,
        "catalogue_digest": catalogue_digest,
        "normalizer_registry_digest": normalizer_registry_digest,
        "prepared": _prepared_projection(prepared),
        "reconciliations": [None if item.result is None else item.result.to_dict() for item in prepared],
        "prompt_context_digest": None if not prompt_context else content_digest(prompt_context),
        "execution_status": execution_status,
        "route_digest": route_digest,
        "execution_digest": execution_digest,
        "retention": _RETENTION,
        "secret_material": "never_returned",
    }
    result_digest = content_digest(descriptor)
    result = AutonomousDomainEvidenceBrainRunResult(
        status=status,
        task_digest=task_digest,
        execution_plan_digest=execution_plan_digest,
        evidence_plan=plan,
        prepared=tuple(prepared),
        prompt_context=dict(prompt_context),
        execution=execution,
        execution_status=execution_status,
        route_digest=route_digest,
        execution_digest=execution_digest,
        catalogue_digest=catalogue_digest,
        normalizer_registry_digest=normalizer_registry_digest,
        result_digest=result_digest,
    )
    if len(json.dumps(result.to_dict(), ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")) > MAX_AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_RESULT_BYTES:
        raise BrainRunError("domain evidence brain result exceeds its bound")
    return result


def run_autonomous_domain_evidence_backed(
    agent: Any,
    *,
    task: str,
    catalogue: AutonomousDomainEvidenceSourceCatalogue,
    credentials: Any,
    domains: Sequence[str] | None = None,
    model_candidates: Sequence[Any] | None = None,
    available_evidence: Sequence[str] = (),
    completed_stages: Mapping[str, Sequence[str]] | None = None,
    prepare_options: Mapping[str, Any] | None = None,
    prepare_for_requirement: Callable[[AutonomousEvidenceRequirement], Mapping[str, Any]] | None = None,
    execute_options: Mapping[str, Any] | None = None,
    max_parallel_requirements: int | None = None,
    allow_incomplete_evidence: bool = False,
    approve_source_dispatch: bool = False,
    approve_provider_call: bool = False,
    provider_run_override: Any | None = None,
    before_provider_run: Callable[[AutonomousDomainEvidenceBrainPreflight], None] | None = None,
    prompt_builder: Callable[[AutonomousDomainEvidenceBrainPromptProjection], Mapping[str, Any]] | None = None,
    run_mode: str = "auto",
    run_options: Mapping[str, Any] | None = None,
) -> AutonomousDomainEvidenceBrainRunResult:
    """Run all selected catalogue evidence requirements through the ordinary autonomous brain."""

    if not hasattr(agent, "evidence_plan") or not callable(agent.evidence_plan):
        raise BrainRunError("domain evidence brain requires an AutonomousAgent")
    if not isinstance(catalogue, AutonomousDomainEvidenceSourceCatalogue):
        raise ArgumentError("domain evidence brain requires an AutonomousDomainEvidenceSourceCatalogue")
    task_text = _bounded_task(task)
    selected_domains = _bounded_domains(domains)
    if run_mode not in {"auto", "domain", "cross_domain"}:
        raise ArgumentError("domain evidence brain run_mode must be auto, domain, or cross_domain")
    if run_mode == "domain" and len(selected_domains) != 1:
        raise ArgumentError("domain evidence brain domain mode requires exactly one domain")
    if run_mode == "cross_domain" and not 2 <= len(selected_domains) <= 8:
        raise ArgumentError("domain evidence brain cross-domain mode requires 2..8 domains")
    if not isinstance(allow_incomplete_evidence, bool) or not isinstance(approve_source_dispatch, bool) or not isinstance(approve_provider_call, bool):
        raise ArgumentError("domain evidence brain approval controls must be booleans")
    prepare_kwargs = _mapping_options("domain evidence brain prepare_options", prepare_options)
    execute_kwargs = _mapping_options("domain evidence brain execute_options", execute_options)
    if "approve_source_dispatch" in execute_kwargs:
        raise ArgumentError("domain evidence brain execute_options cannot override approve_source_dispatch")
    options = _mapping_options("domain evidence brain run_options", run_options)
    reserved = {"task", "domain", "subtasks", "credentials", "model_candidates", "approve_provider_call", "approve_source_dispatch"}
    forbidden = sorted(reserved.intersection(options))
    if forbidden:
        raise ArgumentError("domain evidence brain run_options cannot override: " + ", ".join(forbidden))
    if prepare_for_requirement is not None and not callable(prepare_for_requirement):
        raise ArgumentError("domain evidence brain prepare_for_requirement must be callable or None")
    if before_provider_run is not None and not callable(before_provider_run):
        raise ArgumentError("domain evidence brain before_provider_run must be callable or None")
    if prompt_builder is not None and not callable(prompt_builder):
        raise ArgumentError("domain evidence brain prompt_builder must be callable or None")

    plan = agent.evidence_plan(selected_domains, available_evidence=available_evidence, completed_stages=completed_stages)
    if not isinstance(plan, AutonomousEvidencePlan) or not 1 <= len(plan.requirements) <= MAX_AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_REQUIREMENTS:
        raise ArgumentError("domain evidence brain plan requirements are outside their bound")
    catalogue_digest = catalogue.registry_digest
    normalizer_registry_digest = catalogue.normalizer_registry.registry_digest
    prepared: list[AutonomousDomainEvidenceBrainPreparation] = []
    for requirement in plan.requirements:
        per_requirement = {} if prepare_for_requirement is None else _mapping_options(
            "domain evidence brain requirement preparation", prepare_for_requirement(requirement),
        )
        prepared.append(AutonomousDomainEvidenceBrainPreparation(
            requirement_id=requirement.requirement_id,
            domain=requirement.domain,
            prepared=catalogue.prepare(plan, requirement.requirement_id, **{**prepare_kwargs, **per_requirement}),
        ))
    task_digest = _safe_task_digest(task_text)
    execution_plan_digest = content_digest({
        "schema": AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_RUN_SCHEMA,
        "task_digest": task_digest,
        "evidence_plan_digest": plan.plan_digest,
        "catalogue_digest": catalogue_digest,
        "normalizer_registry_digest": normalizer_registry_digest,
        "domains": list(selected_domains),
        "run_mode": run_mode,
    })
    if not approve_source_dispatch:
        return _build_result(
            status="evidence_review_required", task_digest=task_digest, execution_plan_digest=execution_plan_digest, plan=plan, prepared=prepared,
            prompt_context={}, execution=None, execution_status=None, route_digest=None, execution_digest=None,
            catalogue_digest=catalogue_digest, normalizer_registry_digest=normalizer_registry_digest,
        )
    if catalogue.registry_digest != catalogue_digest:
        raise ArgumentError("domain evidence catalogue changed after preparation; review is required again")
    parallel = max_parallel_requirements
    if parallel is None:
        parallel = min(MAX_AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_PARALLEL_REQUIREMENTS, len(prepared))
    if not isinstance(parallel, int) or isinstance(parallel, bool) or not 1 <= parallel <= MAX_AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_PARALLEL_REQUIREMENTS:
        raise ArgumentError("domain evidence brain max_parallel_requirements is outside its bound")

    def execute_one(item: AutonomousDomainEvidenceBrainPreparation) -> AutonomousEvidenceReconciliationResult:
        return catalogue.execute(plan, item.prepared, approve_source_dispatch=True, **execute_kwargs)

    with ThreadPoolExecutor(max_workers=min(parallel, len(prepared)), thread_name_prefix="aurora-domain-evidence") as executor:
        futures = tuple(executor.submit(execute_one, item) for item in prepared)
        results = tuple(future.result() for future in futures)
    prepared = [AutonomousDomainEvidenceBrainPreparation(item.requirement_id, item.domain, item.prepared, result) for item, result in zip(prepared, results)]
    complete = all(item.result is not None and item.result.status in {"consensus", "consensus_with_dissent"} for item in prepared)
    if not complete and not allow_incomplete_evidence:
        return _build_result(
            status=_status_for_evidence(prepared), task_digest=task_digest, execution_plan_digest=execution_plan_digest, plan=plan, prepared=prepared,
            prompt_context={}, execution=None, execution_status=None, route_digest=None, execution_digest=None,
            catalogue_digest=catalogue_digest, normalizer_registry_digest=normalizer_registry_digest,
        )
    values = {item.requirement_id: dict(item.result.values) if item.result is not None else {} for item in prepared}
    normalized_values = {item.requirement_id: dict(item.result.normalized_values) if item.result is not None else {} for item in prepared}
    projection = AutonomousDomainEvidenceBrainPromptProjection(plan, tuple(prepared), values, normalized_values)
    prompt_context = _safe_context(_default_prompt_context(plan, prepared) if prompt_builder is None else prompt_builder(projection))
    existing_context = options.get("context")
    if existing_context is not None and not isinstance(existing_context, Mapping):
        raise ArgumentError("domain evidence brain run_options.context must be a mapping")
    merged_context = _safe_context(
        {} if existing_context is None else existing_context,
        "domain evidence brain caller context",
    )
    conflicts = sorted(set(merged_context).intersection(prompt_context))
    if conflicts:
        raise ArgumentError("domain evidence brain prompt context cannot override caller context: " + ", ".join(conflicts))
    merged_context.update(prompt_context)
    options["context"] = merged_context
    options["approve_provider_call"] = approve_provider_call
    options["domain_policy_evidence_ready"] = True
    if provider_run_override is not None:
        if approve_provider_call is not True:
            raise ArgumentError("domain evidence brain provider_run_override requires provider approval")
        execution = provider_run_override
    else:
        if before_provider_run is not None:
            before_provider_run(AutonomousDomainEvidenceBrainPreflight(plan, tuple(prepared), dict(prompt_context)))
        if run_mode == "domain":
            execution = agent.run(task=task_text, domain=selected_domains[0], credentials=credentials, model_candidates=model_candidates, **options)
        elif run_mode == "cross_domain":
            subtasks = tuple({"id": f"evidence-{domain}", "domain": domain, "task": task_text} for domain in selected_domains)
            execution = agent.run_cross_domain(task=task_text, subtasks=subtasks, credentials=credentials, model_candidates=model_candidates, **options)
        else:
            execution = agent.run_auto(task=task_text, credentials=credentials, model_candidates=model_candidates, **options)
    execution_status, route_digest, execution_digest = _execution_metadata(agent, execution)
    final_status = (
        "completed" if isinstance(execution_status, str) and (execution_status.startswith("completed") or execution_status in {"children_completed", "succeeded"})
        else "provider_review_required" if isinstance(execution_status, str) and (execution_status == "approval_required" or execution_status.endswith("review_required"))
        else "provider_failed"
    )
    return _build_result(
        status=final_status, task_digest=task_digest, execution_plan_digest=execution_plan_digest, plan=plan, prepared=prepared, prompt_context=prompt_context,
        execution=execution, execution_status=execution_status, route_digest=route_digest, execution_digest=execution_digest,
        catalogue_digest=catalogue_digest, normalizer_registry_digest=normalizer_registry_digest,
    )


__all__ = [
    "AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_RUN_SCHEMA",
    "AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_CONTEXT_SCHEMA",
    "AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_STATUSES",
    "MAX_AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_REQUIREMENTS",
    "MAX_AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_PARALLEL_REQUIREMENTS",
    "MAX_AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_CONTEXT_BYTES",
    "MAX_AUTONOMOUS_DOMAIN_EVIDENCE_BRAIN_RESULT_BYTES",
    "AutonomousDomainEvidenceBrainPreparation",
    "AutonomousDomainEvidenceBrainPromptProjection",
    "AutonomousDomainEvidenceBrainPreflight",
    "AutonomousDomainEvidenceBrainRunResult",
    "run_autonomous_domain_evidence_backed",
]
