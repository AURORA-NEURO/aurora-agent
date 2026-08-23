"""Offline autonomy scenario matrices for provider-neutral integration testing and replay.

The scenario runner is intentionally an application-facing composition seam. It uses the real
selection preview and exact approval handoff, invokes only a caller-registered local provider,
requires caller-owned value evidence, and settles reward through the existing brain outcome
record boundary. Reports contain digests and bounded evaluator metadata only.
"""

from __future__ import annotations

import hashlib
import json
from typing import Any, Callable, Mapping, Sequence

from .autonomy import AUTONOMOUS_DOMAINS, AutonomousAgent
from .brain import BrainRunError
from .evaluators import DomainEvaluatorRegistry
from .authoring import content_digest


AUTONOMOUS_OFFLINE_SCENARIO_SCHEMA = "bioprism-autonomous-offline-scenario/0.1"
AUTONOMOUS_OFFLINE_SCENARIO_REPLAY_SCHEMA = "bioprism-autonomous-offline-scenario-replay/0.1"
MAX_AUTONOMOUS_OFFLINE_SCENARIO_CASES = len(AUTONOMOUS_DOMAINS)
MAX_AUTONOMOUS_OFFLINE_SCENARIO_BYTES = 750_000
_DIGEST_LENGTH = 64


def _digest(value: Any) -> str:
    return hashlib.sha256(
        json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")
    ).hexdigest()


def _text(name: str, value: Any, maximum: int = 32_000) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise BrainRunError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > maximum:
        raise BrainRunError(f"{name} exceeds its bounded size")
    return value


def _digest_field(name: str, value: Any) -> str:
    if not isinstance(value, str) or len(value) != _DIGEST_LENGTH or any(char not in "0123456789abcdef" for char in value):
        raise BrainRunError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _normalize_case(value: Mapping[str, Any], index: int) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise BrainRunError("offline scenario cases must be mappings")
    domain = value.get("domain")
    if not isinstance(domain, str) or domain not in AUTONOMOUS_DOMAINS:
        raise BrainRunError("offline scenario case domain must be a built-in domain")
    task = _text("offline scenario task", value.get("task"))
    case_id = value.get("id", f"{domain}-{index + 1}")
    case_id = _text("offline scenario case id", case_id, 256)
    normalized: dict[str, Any] = {"id": case_id, "domain": domain, "task": task}
    for key, maximum in (("capability", 256),):
        if value.get(key) is not None:
            normalized[key] = _text(f"offline scenario {key}", value[key], maximum)
    for key in ("context", "candidates", "evidence"):
        if value.get(key) is not None:
            normalized[key] = json.loads(json.dumps(value[key], ensure_ascii=False, allow_nan=False))
    return normalized


def _execution_metadata(result: Any) -> dict[str, Any]:
    response = getattr(result, "response", None)
    provider = getattr(response, "provider", None)
    model = getattr(response, "model", None)
    request_id = getattr(response, "request_id", None)
    return {
        "status": _text("offline scenario execution status", getattr(result, "status", "unknown"), 128),
        "selected_model": {"provider": provider or "unknown", "model": model or "unknown"},
        "provider_request_id": request_id if isinstance(request_id, str) else None,
        "response_metadata": None
        if not isinstance(provider, str) or not isinstance(model, str)
        else {
            "provider": provider,
            "model": model,
            "request_id": request_id if isinstance(request_id, str) else None,
            "structured": getattr(response, "structured", None) is not None,
            "tool_call_count": len(getattr(response, "tool_calls", ()) or ()),
        },
        "secret_material": "never_returned",
    }


def _decision_projection(decision: Any) -> dict[str, Any]:
    return {
        "evaluator_id": decision.evaluator_id,
        "evaluator_version": decision.evaluator_version,
        "reward": decision.reward,
        "passed": decision.passed,
        "failed": decision.failed,
        "failure_class": decision.failure_class,
        "feedback_digest": decision.feedback_digest,
        "evidence_digest": decision.evidence_digest,
        "evaluation_digest": _digest(decision.to_dict()),
    }


def _report_without_digest(report: Mapping[str, Any]) -> dict[str, Any]:
    return {key: value for key, value in report.items() if key != "report_digest"}


def _validate_report(report: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(report, Mapping) or report.get("schema") != AUTONOMOUS_OFFLINE_SCENARIO_SCHEMA:
        raise BrainRunError("offline scenario report schema is invalid")
    cases = report.get("cases")
    if not isinstance(cases, Sequence) or isinstance(cases, (str, bytes)) or len(cases) > MAX_AUTONOMOUS_OFFLINE_SCENARIO_CASES:
        raise BrainRunError("offline scenario report cases are outside their bound")
    _digest_field("offline scenario report_digest", report.get("report_digest"))
    if _digest(_report_without_digest(report)) != report["report_digest"]:
        raise BrainRunError("offline scenario report digest does not match its metadata")
    seen: set[str] = set()
    for row in cases:
        if not isinstance(row, Mapping):
            raise BrainRunError("offline scenario report case is malformed")
        domain = row.get("domain")
        if not isinstance(domain, str) or domain not in AUTONOMOUS_DOMAINS or domain in seen:
            raise BrainRunError("offline scenario report domains must be unique built-in domains")
        seen.add(domain)
        for field in ("task_digest", "selection_digest", "selection_contract_digest"):
            _digest_field(f"offline scenario {field}", row.get(field))
        learning = row.get("learning")
        if not isinstance(learning, Mapping):
            raise BrainRunError("offline scenario learning projection is malformed")
        for field in ("outcome_digest", "contract_digest"):
            if learning.get(field) is not None:
                _digest_field(f"offline scenario {field}", learning[field])
        evaluation = row.get("evaluation")
        if evaluation is not None:
            if not isinstance(evaluation, Mapping):
                raise BrainRunError("offline scenario evaluation projection is malformed")
            for field in ("evaluation_digest", "evidence_digest", "feedback_digest"):
                if evaluation.get(field) is not None:
                    _digest_field(f"offline scenario {field}", evaluation[field])
    try:
        encoded = json.dumps(dict(report), ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise BrainRunError("offline scenario report must be JSON-safe") from error
    if len(encoded) > MAX_AUTONOMOUS_OFFLINE_SCENARIO_BYTES:
        raise BrainRunError("offline scenario report exceeds its bounded size")
    return json.loads(encoded.decode("utf-8"))


class AutonomousOfflineScenarioHarness:
    """Run and replay a metadata-only all-domain selection/evaluation matrix.

    ``evidence_for`` receives only preview and execution metadata. It never receives provider
    response text, prompt messages, credentials, or raw tool envelopes. ``bandit_state`` is
    caller-owned and is advanced only by ``brain_outcome_record`` after explicit evaluation.
    """

    def __init__(
        self,
        agent: AutonomousAgent,
        *,
        evaluator_registry: DomainEvaluatorRegistry | None = None,
    ) -> None:
        if not isinstance(agent, AutonomousAgent):
            raise BrainRunError("offline scenario harness requires an AutonomousAgent")
        if evaluator_registry is not None and not isinstance(evaluator_registry, DomainEvaluatorRegistry):
            raise BrainRunError("offline scenario evaluator_registry must be a DomainEvaluatorRegistry")
        self.agent = agent
        self.evaluator_registry = evaluator_registry or DomainEvaluatorRegistry.with_builtin_autonomous_profiles()

    def run(
        self,
        cases: Sequence[Mapping[str, Any]],
        *,
        credentials: Mapping[str, Any] | Any,
        evidence_for: Callable[[Mapping[str, Any]], Mapping[str, Any]] | None = None,
        bandit_state: Mapping[str, Any] | None = None,
        evaluator_registry: DomainEvaluatorRegistry | None = None,
    ) -> dict[str, Any]:
        if not isinstance(cases, Sequence) or isinstance(cases, (str, bytes)) or not 1 <= len(cases) <= MAX_AUTONOMOUS_OFFLINE_SCENARIO_CASES:
            raise BrainRunError("offline scenario cases must contain 1..12 entries")
        normalized = [_normalize_case(item, index) for index, item in enumerate(cases)]
        domains = [item["domain"] for item in normalized]
        if len(set(domains)) != len(domains):
            raise BrainRunError("offline scenario cases must contain at most one case per domain")
        if evidence_for is not None and not callable(evidence_for):
            raise BrainRunError("offline scenario evidence_for must be callable")
        registry = evaluator_registry or self.evaluator_registry
        if not isinstance(registry, DomainEvaluatorRegistry):
            raise BrainRunError("offline scenario evaluator registry is malformed")
        state: Mapping[str, Any] = (
            {"schema": "bioprism-brain-bandit/0.1", "generation": 0, "arms": []}
            if bandit_state is None
            else dict(bandit_state)
        )
        rows: list[dict[str, Any]] = []
        for case in normalized:
            preview = self.agent.model_selection_preview(
                task=case["task"],
                domain=case["domain"],
                credentials=credentials,
                model_candidates=case.get("candidates"),
                capability=case.get("capability"),
                context=case.get("context"),
                bandit_state=state,
            )
            selection_audit = preview.get("selection_audit")
            if not isinstance(selection_audit, Mapping):
                raise BrainRunError("offline scenario selection audit is malformed")
            selected = selection_audit.get("selected_model")
            selection_digest = selection_audit.get("decision_digest")
            if not isinstance(selection_digest, str):
                selection_digest = _digest(selection_audit)
            selection_contract = preview.get("selection_contract")
            if not isinstance(selection_contract, Mapping):
                raise BrainRunError("offline scenario selection contract is malformed")
            base_row: dict[str, Any] = {
                "id": case["id"],
                "domain": case["domain"],
                "task_digest": preview.get("task_digest"),
                "selected_model": None,
                "selection_digest": selection_digest,
                "selection_contract_digest": content_digest(selection_contract),
                "execution": {"status": None, "provider_request_id": None},
                "evaluation": None,
                "learning": {"arm_id": None, "outcome_digest": None, "contract_digest": None, "generation": state.get("generation")},
                "retention": "metadata_only;task_prompt_response_credentials_and_evidence_not_retained",
                "secret_material": "never_returned",
            }
            _digest_field("offline scenario task_digest", base_row["task_digest"])
            if preview.get("status") != "selected":
                base_row["status"] = "selection_refused"
                rows.append(base_row)
                continue
            if not isinstance(selected, Mapping) or not isinstance(selected.get("provider"), str) or not isinstance(selected.get("model"), str):
                raise BrainRunError("offline scenario selected model metadata is malformed")
            result = self.agent.run_approved_model_selection(
                task=case["task"],
                domain=case["domain"],
                selection_preview=preview,
                credentials=credentials,
                model_candidates=case.get("candidates"),
                capability=case.get("capability"),
                context=case.get("context"),
                bandit_state=state,
            )
            execution = _execution_metadata(result)
            evidence = case.get("evidence")
            if evidence_for is not None:
                evidence = evidence_for({"case": case, "preview": preview, "execution": execution})
            if not isinstance(evidence, Mapping):
                raise BrainRunError(f"offline scenario {case['domain']} requires caller-owned evaluation evidence")
            adapter = registry.resolve_for_autonomous_domain(case["domain"])
            decision = adapter.assess_value_only_input({
                "schema": "bioprism-brain-evaluator-input/0.1",
                "context": {"domain": case["domain"]},
                "evidence": dict(evidence),
            })
            evaluation = _decision_projection(decision)
            outcome_digest = getattr(result, "outcome_digest", None)
            if not isinstance(outcome_digest, str):
                raise BrainRunError("offline scenario execution did not produce an outcome digest")
            contract_digest = content_digest(selection_contract)
            settlement = self.agent.brain.record_evaluator_outcome(
                result,
                bandit_state=state,
                evaluator_id=decision.evaluator_id,
                evaluator_version=decision.evaluator_version,
                reward=decision.reward,
                passed=decision.passed,
                failed=decision.failed,
                feedback_digest=decision.feedback_digest,
                failure_class=decision.failure_class,
                evidence_digest=decision.evidence_digest,
                replay_metadata={
                    "scenario_schema": AUTONOMOUS_OFFLINE_SCENARIO_SCHEMA,
                    "domain": case["domain"],
                    "selection_contract_digest": contract_digest,
                },
            )
            next_state = settlement.get("next_state") if isinstance(settlement, Mapping) else None
            if not isinstance(next_state, Mapping):
                raise BrainRunError("offline scenario settlement returned no next bandit state")
            state = dict(next_state)
            base_row.update(
                {
                    "status": "completed",
                    "selected_model": {"provider": selected["provider"], "model": selected["model"]},
                    "execution": {"status": execution["status"], "provider_request_id": execution["provider_request_id"]},
                    "evaluation": evaluation,
                    "learning": {
                        "arm_id": f"{selected['provider']}/{selected['model']}",
                        "outcome_digest": outcome_digest,
                        "contract_digest": contract_digest,
                        "generation": state.get("generation"),
                    },
                }
            )
            rows.append(base_row)
        descriptor = {
            "schema": AUTONOMOUS_OFFLINE_SCENARIO_SCHEMA,
            "status": "completed" if all(row["status"] == "completed" for row in rows) else "partial",
            "case_count": len(rows),
            "completed_count": sum(row["status"] == "completed" for row in rows),
            "refused_count": sum(row["status"] == "selection_refused" for row in rows),
            "domains": [row["domain"] for row in rows],
            "cases": rows,
            "evaluator_catalogue_digest": content_digest(registry.catalogue()),
            "learning_state_digest": content_digest(state),
            "learning_generation": state.get("generation"),
            "execution": "offline_provider_invocation_allowed;external_network_not_required",
            "retention": "metadata_only;task_prompt_response_credentials_and_evidence_not_retained",
            "secret_material": "never_returned",
        }
        report = {**descriptor, "report_digest": content_digest(descriptor)}
        encoded = json.dumps(report, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")
        if len(encoded) > MAX_AUTONOMOUS_OFFLINE_SCENARIO_BYTES:
            raise BrainRunError("offline scenario report exceeds its bounded size")
        return json.loads(encoded.decode("utf-8"))

    def run_all(
        self,
        *,
        credentials: Mapping[str, Any] | Any,
        tasks: Mapping[str, str] | None = None,
        task_for_domain: Callable[[str], str] | None = None,
        evidence_for: Callable[[Mapping[str, Any]], Mapping[str, Any]],
        capability_for_domain: Callable[[str], str | None] | None = None,
        context_for_domain: Callable[[str], Mapping[str, Any] | None] | None = None,
        candidates_for_domain: Callable[[str], Sequence[Mapping[str, Any]] | None] | None = None,
        bandit_state: Mapping[str, Any] | None = None,
        evaluator_registry: DomainEvaluatorRegistry | None = None,
    ) -> dict[str, Any]:
        if not callable(evidence_for):
            raise BrainRunError("offline scenario run_all requires evidence_for")
        task_map = {} if tasks is None else dict(tasks)
        cases = []
        for domain in AUTONOMOUS_DOMAINS:
            task = task_for_domain(domain) if task_for_domain is not None else task_map.get(domain, f"perform a bounded offline {domain} evaluation")
            case: dict[str, Any] = {"id": domain, "domain": domain, "task": task}
            if capability_for_domain is not None and capability_for_domain(domain) is not None:
                case["capability"] = capability_for_domain(domain)
            if context_for_domain is not None and context_for_domain(domain) is not None:
                case["context"] = context_for_domain(domain)
            if candidates_for_domain is not None and candidates_for_domain(domain) is not None:
                case["candidates"] = candidates_for_domain(domain)
            cases.append(case)
        return self.run(
            cases,
            credentials=credentials,
            evidence_for=evidence_for,
            bandit_state=bandit_state,
            evaluator_registry=evaluator_registry,
        )

    def replay(
        self,
        report: Mapping[str, Any],
        *,
        evaluator_registry: DomainEvaluatorRegistry | None = None,
    ) -> dict[str, Any]:
        verified = _validate_report(report)
        registry = evaluator_registry or self.evaluator_registry
        if not isinstance(registry, DomainEvaluatorRegistry):
            raise BrainRunError("offline scenario replay evaluator registry is malformed")
        verified_count = 0
        replay_rows = []
        for row in verified["cases"]:
            if row.get("evaluation") is None or row.get("selected_model") is None:
                continue
            evaluation = row["evaluation"]
            registry.resolve_for_replay(
                row["domain"],
                evaluator_id=evaluation["evaluator_id"],
                evaluator_version=evaluation["evaluator_version"],
            )
            _digest_field("offline scenario replay outcome_digest", row["learning"].get("outcome_digest"))
            _digest_field("offline scenario replay contract_digest", row["learning"].get("contract_digest"))
            verified_count += 1
            replay_rows.append({
                "domain": row["domain"],
                "outcome_digest": row["learning"]["outcome_digest"],
                "evaluation_digest": evaluation["evaluation_digest"],
            })
        descriptor = {
            "schema": AUTONOMOUS_OFFLINE_SCENARIO_REPLAY_SCHEMA,
            "source_report_digest": verified["report_digest"],
            "case_count": verified["case_count"],
            "verified_count": verified_count,
            "replayed_count": 0,
            "learner_generation_before": verified.get("learning_generation"),
            "learner_generation_after": verified.get("learning_generation"),
            "idempotent": True,
            "execution": "metadata_only;no_provider_or_tool_invocation",
            "retention": "metadata_only;task_prompt_response_credentials_and_evidence_not_retained",
            "secret_material": "never_returned",
        }
        return {**descriptor, "replay_digest": content_digest({**descriptor, "rows": replay_rows})}


__all__ = [
    "AUTONOMOUS_OFFLINE_SCENARIO_SCHEMA",
    "AUTONOMOUS_OFFLINE_SCENARIO_REPLAY_SCHEMA",
    "MAX_AUTONOMOUS_OFFLINE_SCENARIO_CASES",
    "MAX_AUTONOMOUS_OFFLINE_SCENARIO_BYTES",
    "AutonomousOfflineScenarioHarness",
]
