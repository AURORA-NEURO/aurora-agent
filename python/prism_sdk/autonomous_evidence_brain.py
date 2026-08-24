"""High-level reviewed-evidence composition for the Python autonomous agent.

The lower-level evidence runtime intentionally does not know how a model should be invoked.
This module closes that gap without collapsing authorization boundaries.  It binds a reviewed
evidence plan to caller-owned acquisition/evaluation adapters, projects the accepted evidence
into a transient provider context, and then delegates model selection and invocation to the
ordinary :class:`~prism_sdk.autonomy.AutonomousAgent` paths.

Three decisions remain independently visible:

* source dispatch requires ``approve_source_dispatch``;
* evidence must be accepted unless ``allow_incomplete_evidence`` is explicitly enabled; and
* provider invocation requires ``approve_provider_call`` and the existing agent gates.

The result is intentionally metadata-only when serialized.  ``evidence.values``, prompt
context, and the provider result remain available to the initiating caller but are never copied
into the durable projection or journal by this composition layer.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Callable, Mapping, Sequence

from .authoring import content_digest
from .autonomous_evidence import AutonomousEvidencePlan
from .autonomous_evidence_runtime import (
    AutonomousEvidenceRuntime,
    AutonomousEvidenceRuntimeJournal,
    AutonomousEvidenceRuntimeResult,
)
from .errors import ArgumentError
from .brain import BrainRunError


AUTONOMOUS_EVIDENCE_BACKED_RUN_SCHEMA = "bioprism-python-autonomous-evidence-backed-run/0.1"
AUTONOMOUS_EVIDENCE_BACKED_RUN_STATUSES = (
    "evidence_review_required",
    "evidence_incomplete",
    "evidence_failed",
    "evidence_reconciliation_required",
    "provider_review_required",
    "provider_failed",
    "completed",
)
MAX_AUTONOMOUS_EVIDENCE_BACKED_PROMPT_BYTES = 2_000_000
MAX_AUTONOMOUS_EVIDENCE_BACKED_DOMAINS = 16
MAX_AUTONOMOUS_EVIDENCE_BACKED_CROSS_DOMAIN_SUBTASKS = 8


def _bounded_task(value: Any) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > 32_000:
        raise ArgumentError("evidence-backed task must be bounded non-empty text")
    return value.strip()


def _bounded_domains(value: Any, default: Sequence[str]) -> tuple[str, ...]:
    selected = default if value is None else value
    if not isinstance(selected, Sequence) or isinstance(selected, (str, bytes, bytearray)):
        raise ArgumentError("evidence-backed domains must be a sequence")
    if not 1 <= len(selected) <= MAX_AUTONOMOUS_EVIDENCE_BACKED_DOMAINS:
        raise ArgumentError("evidence-backed domains must contain 1..16 entries")
    result: list[str] = []
    for index, domain in enumerate(selected):
        if not isinstance(domain, str) or not domain.strip() or len(domain.encode("utf-8")) > 256:
            raise ArgumentError(f"evidence-backed domain {index} is malformed")
        normalized = domain.strip()
        if normalized in result:
            raise ArgumentError("evidence-backed domains must not contain duplicates")
        result.append(normalized)
    return tuple(result)


def _bounded_requests(value: Any) -> tuple[Mapping[str, Any], ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)) or not value:
        raise ArgumentError("evidence-backed requests must contain at least one mapping")
    if len(value) > 128:
        raise ArgumentError("evidence-backed requests exceed the 128-request bound")
    if any(not isinstance(item, Mapping) for item in value):
        raise ArgumentError("evidence-backed requests must contain mappings")
    return tuple(dict(item) for item in value)


def _json_safe_context(value: Any) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise ArgumentError("evidence-backed prompt builder must return a mapping")
    try:
        encoded = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False)
    except (TypeError, ValueError) as error:
        raise ArgumentError("evidence-backed prompt context must be JSON-safe") from error
    if len(encoded.encode("utf-8")) > MAX_AUTONOMOUS_EVIDENCE_BACKED_PROMPT_BYTES:
        raise ArgumentError("evidence-backed prompt context exceeds its byte bound")
    return dict(value)


def _result_status(evidence_status: str) -> str:
    if evidence_status == "reconciliation_required":
        return "evidence_reconciliation_required"
    if evidence_status == "failed":
        return "evidence_failed"
    return "evidence_incomplete"


def _execution_metadata(agent: Any, execution: Any) -> tuple[str | None, str | None, str | None]:
    """Return status, route digest, and a payload-free execution digest."""

    if execution is None:
        return None, None, None
    status = getattr(execution, "execution_status", getattr(execution, "status", None))
    normalized_status = status if isinstance(status, str) else None
    route_digest: str | None = None
    route = getattr(execution, "route", None)
    candidate_route_digest = getattr(route, "route_digest", None)
    if isinstance(candidate_route_digest, str) and len(candidate_route_digest) == 64:
        route_digest = candidate_route_digest
    try:
        metadata = agent._trace_execution_metadata(execution)
    except Exception:
        metadata = {
            "status": normalized_status,
            "result_type": execution.__class__.__name__,
        }
    if isinstance(metadata, Mapping):
        candidate = metadata.get("route_digest")
        if isinstance(candidate, str) and len(candidate) == 64:
            route_digest = candidate
        execution_digest = content_digest(
            {
                key: value
                for key, value in metadata.items()
                if key not in {"response", "prompt", "task", "values"}
            }
        )
    else:
        execution_digest = content_digest({"status": normalized_status, "result_type": execution.__class__.__name__})
    return normalized_status, route_digest, execution_digest


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceBackedRunResult:
    """Transient execution envelope with a strict metadata-only projection."""

    status: str
    task_digest: str
    evidence_plan: AutonomousEvidencePlan
    evidence: AutonomousEvidenceRuntimeResult | None
    prompt_context: Mapping[str, Any]
    execution: Any | None
    route_digest: str | None
    execution_status: str | None
    execution_digest: str | None
    result_digest: str

    def to_dict(self) -> dict[str, Any]:
        descriptor: dict[str, Any] = {
            "schema": AUTONOMOUS_EVIDENCE_BACKED_RUN_SCHEMA,
            "status": self.status,
            "task_digest": self.task_digest,
            "evidence_plan_digest": self.evidence_plan.plan_digest,
            "evidence_result_digest": None if self.evidence is None else self.evidence.result_digest,
            "execution_status": self.execution_status,
            "execution_digest": self.execution_digest,
            "route_digest": self.route_digest,
            "retention": "metadata_only;raw_evidence_prompt_values_and_provider_response_caller_owned",
            "secret_material": "never_returned",
        }
        descriptor["result_digest"] = self.result_digest
        return descriptor


def _build_result(
    *,
    status: str,
    task_digest: str,
    evidence_plan: AutonomousEvidencePlan,
    evidence: AutonomousEvidenceRuntimeResult | None,
    prompt_context: Mapping[str, Any],
    execution: Any | None,
    route_digest: str | None,
    execution_status: str | None,
    execution_digest: str | None,
) -> AutonomousEvidenceBackedRunResult:
    descriptor = {
        "schema": AUTONOMOUS_EVIDENCE_BACKED_RUN_SCHEMA,
        "status": status,
        "task_digest": task_digest,
        "evidence_plan_digest": evidence_plan.plan_digest,
        "evidence_result_digest": None if evidence is None else evidence.result_digest,
        "execution_status": execution_status,
        "execution_digest": execution_digest,
        "route_digest": route_digest,
        "retention": "metadata_only;raw_evidence_prompt_values_and_provider_response_caller_owned",
        "secret_material": "never_returned",
    }
    return AutonomousEvidenceBackedRunResult(
        status=status,
        task_digest=task_digest,
        evidence_plan=evidence_plan,
        evidence=evidence,
        prompt_context=dict(prompt_context),
        execution=execution,
        route_digest=route_digest,
        execution_status=execution_status,
        execution_digest=execution_digest,
        result_digest=content_digest(descriptor),
    )


def run_autonomous_evidence_backed(
    agent: Any,
    *,
    task: str,
    requests: Sequence[Mapping[str, Any]],
    acquirer: Any,
    credentials: Any,
    domains: Sequence[str] | None = None,
    model_candidates: Sequence[Any] | None = None,
    projector: Any | None = None,
    evaluator: Any | None = None,
    rehydrate_value: Callable[[Mapping[str, Any]], Any] | None = None,
    parent_evidence_digests: Sequence[str] = (),
    stop_on_failure: bool = False,
    reevaluate_pending: bool = False,
    available_evidence: Sequence[str] = (),
    completed_stages: Mapping[str, Sequence[str]] | None = None,
    journal: AutonomousEvidenceRuntimeJournal | None = None,
    approve_source_dispatch: bool = False,
    allow_incomplete_evidence: bool = False,
    approve_provider_call: bool = False,
    prompt_builder: Callable[[AutonomousEvidenceRuntimeResult], Mapping[str, Any]] | None = None,
    run_mode: str = "auto",
    run_options: Mapping[str, Any] | None = None,
) -> AutonomousEvidenceBackedRunResult:
    """Acquire reviewed evidence, then invoke the existing autonomous execution path.

    ``run_options`` is intentionally a mapping of ordinary agent options rather than a second
    authorization surface.  Task, domain, credentials, model candidates, and the three explicit
    approval controls are reserved and cannot be smuggled through it.
    """

    if not hasattr(agent, "evidence_plan") or not callable(agent.evidence_plan):
        raise BrainRunError("evidence-backed execution requires an AutonomousAgent")
    task_text = _bounded_task(task)
    from .autonomy import AUTONOMOUS_DOMAINS

    selected_domains = _bounded_domains(domains, AUTONOMOUS_DOMAINS)
    if run_mode not in {"auto", "domain", "cross_domain"}:
        raise ArgumentError("evidence-backed run_mode must be auto, domain, or cross_domain")
    if run_mode == "domain" and len(selected_domains) != 1:
        raise ArgumentError("domain evidence-backed execution requires exactly one domain")
    if run_mode == "cross_domain" and not 2 <= len(selected_domains) <= MAX_AUTONOMOUS_EVIDENCE_BACKED_CROSS_DOMAIN_SUBTASKS:
        raise ArgumentError("cross-domain evidence-backed execution requires 2..8 domains")
    if not isinstance(approve_source_dispatch, bool) or not isinstance(allow_incomplete_evidence, bool) or not isinstance(approve_provider_call, bool):
        raise ArgumentError("evidence-backed approval controls must be booleans")
    if not callable(acquirer):
        # Protocol-style objects are supported by the runtime, so only reject an object that has
        # neither the callable form nor the documented acquire method.
        if not callable(getattr(acquirer, "acquire", None)):
            raise ArgumentError("evidence-backed acquirer must be callable or implement acquire")
    if not isinstance(run_options, Mapping) and run_options is not None:
        raise ArgumentError("evidence-backed run_options must be a mapping")
    source_requests = _bounded_requests(requests)
    options = {} if run_options is None else dict(run_options)
    reserved = {
        "task", "domain", "subtasks", "credentials", "model_candidates", "execution_id",
        "approve_provider_call", "approve_source_dispatch",
    }
    forbidden = sorted(reserved.intersection(options))
    if forbidden:
        raise ArgumentError("evidence-backed run_options cannot override: " + ", ".join(forbidden))

    plan = agent.evidence_plan(
        selected_domains,
        available_evidence=available_evidence,
        completed_stages=completed_stages,
    )
    task_digest = content_digest({"task": task_text})
    empty_context: dict[str, Any] = {}
    if not approve_source_dispatch:
        return _build_result(
            status="evidence_review_required",
            task_digest=task_digest,
            evidence_plan=plan,
            evidence=None,
            prompt_context=empty_context,
            execution=None,
            route_digest=None,
            execution_status=None,
            execution_digest=None,
        )

    runtime = AutonomousEvidenceRuntime(plan, journal=journal)
    runtime.rehydrate()
    evidence = runtime.execute(
        source_requests,
        acquirer=acquirer,
        projector=projector,
        evaluator=evaluator,
        rehydrate_value=rehydrate_value,
        parent_evidence_digests=parent_evidence_digests,
        stop_on_failure=stop_on_failure,
        reevaluate_pending=reevaluate_pending,
    )
    if evidence.status != "completed" and not allow_incomplete_evidence:
        return _build_result(
            status=_result_status(evidence.status),
            task_digest=task_digest,
            evidence_plan=plan,
            evidence=evidence,
            prompt_context=empty_context,
            execution=None,
            route_digest=None,
            execution_status=None,
            execution_digest=None,
        )

    if prompt_builder is None:
        prompt_context: Mapping[str, Any] = {
            "evidence_backed": {
                "schema": AUTONOMOUS_EVIDENCE_BACKED_RUN_SCHEMA,
                "plan_digest": plan.plan_digest,
                "result_digest": evidence.result_digest,
                "status": evidence.status,
                "completed_requirement_ids": list(evidence.completed_requirement_ids),
                "pending_evaluation_requirement_ids": list(evidence.pending_evaluation_requirement_ids),
                "missing_requirement_ids": list(evidence.missing_requirement_ids),
                "retention": "metadata_only;raw_values_caller_owned",
            }
        }
    else:
        try:
            prompt_context = _json_safe_context(prompt_builder(evidence))
        except ArgumentError:
            raise
        except Exception as error:
            raise BrainRunError("evidence-backed prompt builder failed") from error

    options["approve_provider_call"] = approve_provider_call
    existing_context = options.get("context")
    if existing_context is not None and not isinstance(existing_context, Mapping):
        raise ArgumentError("evidence-backed run_options.context must be a mapping")
    merged_context = dict(existing_context or {})
    conflicting_context = sorted(set(merged_context).intersection(prompt_context))
    if conflicting_context:
        raise ArgumentError(
            "evidence-backed prompt context cannot override caller context: "
            + ", ".join(conflicting_context)
        )
    merged_context.update(prompt_context)
    options["context"] = merged_context

    if run_mode == "domain":
        execution = agent.run(
            task=task_text,
            domain=selected_domains[0],
            credentials=credentials,
            model_candidates=model_candidates,
            **options,
        )
    elif run_mode == "cross_domain":
        subtasks = tuple(
            {
                "id": f"evidence-{domain}",
                "domain": domain,
                "task": task_text,
            }
            for domain in selected_domains
        )
        execution = agent.run_cross_domain(
            task=task_text,
            subtasks=subtasks,
            credentials=credentials,
            model_candidates=model_candidates,
            **options,
        )
    else:
        execution = agent.run_auto(
            task=task_text,
            credentials=credentials,
            model_candidates=model_candidates,
            **options,
        )
    execution_status, route_digest, execution_digest = _execution_metadata(agent, execution)
    final_status = (
        "completed"
        if isinstance(execution_status, str)
        and (execution_status.startswith("completed") or execution_status in {"children_completed", "succeeded"})
        else "provider_review_required"
        if isinstance(execution_status, str)
        and (execution_status == "approval_required" or execution_status.endswith("review_required"))
        else "provider_failed"
    )
    return _build_result(
        status=final_status,
        task_digest=task_digest,
        evidence_plan=plan,
        evidence=evidence,
        prompt_context=prompt_context,
        execution=execution,
        route_digest=route_digest,
        execution_status=execution_status,
        execution_digest=execution_digest,
    )


__all__ = [
    "AUTONOMOUS_EVIDENCE_BACKED_RUN_SCHEMA",
    "AUTONOMOUS_EVIDENCE_BACKED_RUN_STATUSES",
    "MAX_AUTONOMOUS_EVIDENCE_BACKED_PROMPT_BYTES",
    "AutonomousEvidenceBackedRunResult",
    "run_autonomous_evidence_backed",
]
