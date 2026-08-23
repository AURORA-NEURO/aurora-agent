"""Provider-free admission gate for promoting a selection policy.

The replay lab measures a policy; this module decides whether that evidence is strong enough
to activate it.  It deliberately performs no learner mutation, provider invocation, credential
lookup, or reward assignment.  The returned projection contains only bounded metrics, reasons,
and digests so it can be stored as a CI or operator decision without retaining task payloads.
"""

from __future__ import annotations

import math
from typing import Any, Mapping

from .authoring import canonical_json, content_digest
from .autonomous_selection_lab import validate_autonomous_selection_lab_report
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError


AUTONOMOUS_SELECTION_PROMOTION_POLICY_SCHEMA = "bioprism-python-autonomous-selection-promotion-policy/0.1"
AUTONOMOUS_SELECTION_PROMOTION_DOMAIN_SCHEMA = "bioprism-python-autonomous-selection-promotion-domain/0.1"
AUTONOMOUS_SELECTION_PROMOTION_SCHEMA = "bioprism-python-autonomous-selection-promotion/0.1"
MAX_AUTONOMOUS_SELECTION_PROMOTION_REASONS = 64
MAX_AUTONOMOUS_SELECTION_PROMOTION_BYTES = 512_000

_EXECUTION = "gate_only;does_not_mutate_learner_or_invoke_provider"
_RETENTION = "metadata_only;selection_metrics_and_digests"
_SECRET_MATERIAL = "never_returned"
_DOMAINS = tuple(AUTONOMOUS_DOMAIN_NAMES)


def _fail(message: str) -> "NoReturn":
    raise ArgumentError(f"autonomous selection promotion {message}")


def _bounded_integer(name: str, value: Any, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        _fail(f"{name} is outside its integer bounds")
    return value


def _bounded_number(name: str, value: Any, minimum: float, maximum: float) -> float | int:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)) or not minimum <= float(value) <= maximum:
        _fail(f"{name} is outside its numeric bounds")
    return value


def _bounded_rate(name: str, value: Any) -> float | int:
    return _bounded_number(name, value, 0, 1)


def _bounded_reasons(name: str, value: Any) -> list[str]:
    if not isinstance(value, list) or len(value) > MAX_AUTONOMOUS_SELECTION_PROMOTION_REASONS:
        _fail(f"{name} must contain at most {MAX_AUTONOMOUS_SELECTION_PROMOTION_REASONS} reasons")
    result: list[str] = []
    for index, reason in enumerate(value):
        if not isinstance(reason, str) or not reason.strip() or len(reason) > 512 or "\x00" in reason:
            _fail(f"{name}[{index}] is invalid")
        result.append(reason)
    return result


def _rounded(value: float) -> float:
    return round(float(value), 12)


def _ratio(numerator: int, denominator: int) -> float:
    return 0.0 if denominator == 0 else _rounded(numerator / denominator)


def _optional_ratio(numerator: int, denominator: int) -> float | None:
    return None if denominator == 0 else _rounded(numerator / denominator)


def _add_reason(reasons: list[str], value: str) -> None:
    if value not in reasons:
        reasons.append(value)


def _policy_projection(
    *,
    require_all_domains: bool,
    min_cases_per_domain: int,
    min_evaluated_cases_per_domain: int,
    min_evaluated_coverage: float,
    min_oracle_agreement_rate: float,
    max_mean_regret: float,
    max_abstention_rate: float,
    max_selected_reward_missing_rate: float,
    max_no_eligible_model_rate: float,
    max_no_counterfactual_reward_rate: float,
) -> dict[str, Any]:
    if not isinstance(require_all_domains, bool):
        _fail("require_all_domains must be boolean")
    _bounded_integer("min_cases_per_domain", min_cases_per_domain, 1, 4_096)
    _bounded_integer("min_evaluated_cases_per_domain", min_evaluated_cases_per_domain, 1, 4_096)
    _bounded_rate("min_evaluated_coverage", min_evaluated_coverage)
    _bounded_rate("min_oracle_agreement_rate", min_oracle_agreement_rate)
    _bounded_number("max_mean_regret", max_mean_regret, 0, 2)
    _bounded_rate("max_abstention_rate", max_abstention_rate)
    _bounded_rate("max_selected_reward_missing_rate", max_selected_reward_missing_rate)
    _bounded_rate("max_no_eligible_model_rate", max_no_eligible_model_rate)
    _bounded_rate("max_no_counterfactual_reward_rate", max_no_counterfactual_reward_rate)
    return {
        "schema": AUTONOMOUS_SELECTION_PROMOTION_POLICY_SCHEMA,
        "require_all_domains": require_all_domains,
        "min_cases_per_domain": min_cases_per_domain,
        "min_evaluated_cases_per_domain": min_evaluated_cases_per_domain,
        "min_evaluated_coverage": min_evaluated_coverage,
        "min_oracle_agreement_rate": min_oracle_agreement_rate,
        "max_mean_regret": max_mean_regret,
        "max_abstention_rate": max_abstention_rate,
        "max_selected_reward_missing_rate": max_selected_reward_missing_rate,
        "max_no_eligible_model_rate": max_no_eligible_model_rate,
        "max_no_counterfactual_reward_rate": max_no_counterfactual_reward_rate,
    }


def _validate_policy_projection(value: Any) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        _fail("report policy must be an object")
    if value.get("schema") != AUTONOMOUS_SELECTION_PROMOTION_POLICY_SCHEMA:
        _fail("report policy schema is invalid")
    if not isinstance(value.get("require_all_domains"), bool):
        _fail("report policy require_all_domains must be boolean")
    for field in ("min_cases_per_domain", "min_evaluated_cases_per_domain"):
        _bounded_integer(f"report policy {field}", value.get(field), 1, 4_096)
    _bounded_rate("report policy min_evaluated_coverage", value.get("min_evaluated_coverage"))
    _bounded_rate("report policy min_oracle_agreement_rate", value.get("min_oracle_agreement_rate"))
    _bounded_number("report policy max_mean_regret", value.get("max_mean_regret"), 0, 2)
    for field in ("max_abstention_rate", "max_selected_reward_missing_rate", "max_no_eligible_model_rate", "max_no_counterfactual_reward_rate"):
        _bounded_rate(f"report policy {field}", value.get(field))
    return dict(value)


def _expected_domain_reasons(value: Mapping[str, Any], policy: Mapping[str, Any]) -> list[str]:
    case_count = int(value["case_count"])
    reasons: list[str] = []
    if case_count == 0:
        if policy["require_all_domains"]:
            reasons.append("domain has no replay cases")
        return reasons
    if case_count < policy["min_cases_per_domain"]:
        reasons.append(f"domain has fewer than {policy['min_cases_per_domain']} replay cases")
    if int(value["evaluated_count"]) < policy["min_evaluated_cases_per_domain"]:
        reasons.append(f"domain has fewer than {policy['min_evaluated_cases_per_domain']} evaluated cases")
    if float(value["evaluated_coverage"]) < policy["min_evaluated_coverage"]:
        reasons.append(f"evaluated coverage is below {policy['min_evaluated_coverage']}")
    agreement = value["oracle_agreement_rate"]
    if agreement is None or float(agreement) < policy["min_oracle_agreement_rate"]:
        reasons.append(f"oracle agreement is below {policy['min_oracle_agreement_rate']}")
    regret = value["mean_regret"]
    if regret is None or float(regret) > policy["max_mean_regret"]:
        reasons.append(f"mean regret exceeds {policy['max_mean_regret']}")
    for field, label, policy_field in (
        ("abstention_rate", "abstention rate", "max_abstention_rate"),
        ("selected_reward_missing_rate", "selected reward missing rate", "max_selected_reward_missing_rate"),
        ("no_eligible_model_rate", "no eligible model rate", "max_no_eligible_model_rate"),
        ("no_counterfactual_reward_rate", "no counterfactual reward rate", "max_no_counterfactual_reward_rate"),
    ):
        if float(value[field]) > policy[policy_field]:
            reasons.append(f"{label} exceeds {policy[policy_field]}")
    return reasons


def _validate_domain_report(value: Any, index: int, policy: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        _fail(f"report domain {index} must be an object")
    if value.get("schema") != AUTONOMOUS_SELECTION_PROMOTION_DOMAIN_SCHEMA:
        _fail(f"report domain {index} schema is invalid")
    domain = value.get("domain")
    if domain not in _DOMAINS:
        _fail(f"report domain {index} is not supported")
    case_count = _bounded_integer(f"report domain {domain}.case_count", value.get("case_count"), 0, 4_096)
    evaluated_count = _bounded_integer(f"report domain {domain}.evaluated_count", value.get("evaluated_count"), 0, 4_096)
    evaluated_coverage = _bounded_rate(f"report domain {domain}.evaluated_coverage", value.get("evaluated_coverage"))
    oracle_agreement_count = _bounded_integer(f"report domain {domain}.oracle_agreement_count", value.get("oracle_agreement_count"), 0, 4_096)
    if evaluated_count > case_count or oracle_agreement_count > evaluated_count:
        _fail(f"report domain {domain} counts are inconsistent")
    oracle_agreement_rate = value.get("oracle_agreement_rate")
    if oracle_agreement_rate is not None:
        _bounded_rate(f"report domain {domain}.oracle_agreement_rate", oracle_agreement_rate)
    mean_regret = value.get("mean_regret")
    if mean_regret is not None:
        _bounded_number(f"report domain {domain}.mean_regret", mean_regret, 0, 2)
    for field in ("abstention_rate", "selected_reward_missing_rate", "no_eligible_model_rate", "no_counterfactual_reward_rate"):
        _bounded_rate(f"report domain {domain}.{field}", value.get(field))
    if value.get("decision") not in {"admit", "hold", "not_required"}:
        _fail(f"report domain {domain}.decision is invalid")
    reasons = _bounded_reasons(f"report domain {domain}.reasons", value.get("reasons"))
    expected_coverage = 0.0 if case_count == 0 else _rounded(evaluated_count / case_count)
    expected_agreement = _optional_ratio(oracle_agreement_count, evaluated_count)
    if evaluated_coverage != expected_coverage or oracle_agreement_rate != expected_agreement:
        _fail(f"report domain {domain} coverage metrics are inconsistent")
    if value.get("decision") == "not_required" and (case_count != 0 or reasons):
        _fail(f"report domain {domain} not_required decision is inconsistent")
    expected_reasons = _expected_domain_reasons(value, policy)
    if reasons != expected_reasons:
        _fail(f"report domain {domain} reasons do not match its policy metrics")
    expected_decision = "not_required" if case_count == 0 and not policy["require_all_domains"] else ("admit" if not expected_reasons else "hold")
    if value.get("decision") != expected_decision:
        _fail(f"report domain {domain} decision does not match its policy metrics")
    return dict(value)


def validate_autonomous_selection_promotion_report(value: Mapping[str, Any]) -> dict[str, Any]:
    """Validate a promotion report's bounds, decision invariants, and canonical digest."""

    if not isinstance(value, Mapping):
        _fail("report must be an object")
    try:
        encoded = canonical_json(value).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise ArgumentError("autonomous selection promotion report is not canonical JSON") from error
    if len(encoded) > MAX_AUTONOMOUS_SELECTION_PROMOTION_BYTES:
        _fail(f"report exceeds {MAX_AUTONOMOUS_SELECTION_PROMOTION_BYTES} bytes")
    if value.get("schema") != AUTONOMOUS_SELECTION_PROMOTION_SCHEMA:
        _fail("report schema is invalid")
    source_digest = value.get("source_report_digest")
    if not isinstance(source_digest, str) or len(source_digest) != 64 or any(character not in "0123456789abcdef" for character in source_digest):
        _fail("report source_report_digest is malformed")
    policy = _validate_policy_projection(value.get("policy"))
    if value.get("decision") not in {"admit", "hold"}:
        _fail("report decision is invalid")
    reasons = _bounded_reasons("report reasons", value.get("reasons"))
    domains_raw = value.get("domains")
    if not isinstance(domains_raw, list) or len(domains_raw) != len(_DOMAINS):
        _fail("report domains are malformed")
    domains = [_validate_domain_report(row, index, policy) for index, row in enumerate(domains_raw)]
    if tuple(row["domain"] for row in domains) != _DOMAINS:
        _fail("report domains are not in canonical order or contain duplicates")
    if value.get("execution") != _EXECUTION or value.get("retention") != _RETENTION or value.get("secret_material") != _SECRET_MATERIAL:
        _fail("report retention posture is invalid")
    promotion_digest = value.get("promotion_digest")
    if not isinstance(promotion_digest, str) or len(promotion_digest) != 64 or any(character not in "0123456789abcdef" for character in promotion_digest):
        _fail("report promotion_digest is malformed")
    expected_decision = "admit" if not reasons and all(row["decision"] != "hold" for row in domains) else "hold"
    allowed_global_reasons = {"selection replay report is not complete", "selection replay contains no cases"}
    if any(reason not in allowed_global_reasons for reason in reasons):
        _fail("report contains an unknown global reason")
    if all(row["case_count"] == 0 for row in domains) and "selection replay contains no cases" not in reasons:
        _fail("report omits its empty-replay reason")
    if value["decision"] != expected_decision:
        _fail("report decision does not match domain and global reasons")
    if value["decision"] == "admit" and any(row["decision"] == "hold" for row in domains):
        _fail("admitted report contains a held domain")
    body = {key: item for key, item in value.items() if key != "promotion_digest"}
    if content_digest(body) != promotion_digest:
        _fail("report promotion_digest does not match its canonical projection")
    return {
        **dict(value),
        "policy": policy,
        "reasons": reasons,
        "domains": domains,
    }


def _evaluate_domain(row: Mapping[str, Any], policy: Mapping[str, Any]) -> dict[str, Any]:
    case_count = int(row["case_count"])
    evaluated_count = int(row["evaluated_count"])
    reasons: list[str] = []
    has_cases = case_count > 0
    evaluated_coverage = float(row["evaluated_coverage"])
    oracle_agreement_rate = _optional_ratio(int(row["oracle_agreement_count"]), evaluated_count)
    abstention_rate = _ratio(int(row["abstained_count"]), case_count) if "abstained_count" in row else 0.0
    selected_reward_missing_rate = _ratio(int(row["selected_reward_missing_count"]), case_count) if "selected_reward_missing_count" in row else 0.0
    no_eligible_model_rate = _ratio(int(row["no_eligible_model_count"]), case_count) if "no_eligible_model_count" in row else 0.0
    no_counterfactual_reward_rate = _ratio(int(row["no_counterfactual_reward_count"]), case_count) if "no_counterfactual_reward_count" in row else 0.0
    # The source domain row is the validated lab projection, so its counters are available.
    if not has_cases:
        if policy["require_all_domains"]:
            _add_reason(reasons, "domain has no replay cases")
    else:
        if case_count < policy["min_cases_per_domain"]:
            _add_reason(reasons, f"domain has fewer than {policy['min_cases_per_domain']} replay cases")
        if evaluated_count < policy["min_evaluated_cases_per_domain"]:
            _add_reason(reasons, f"domain has fewer than {policy['min_evaluated_cases_per_domain']} evaluated cases")
        if evaluated_coverage < policy["min_evaluated_coverage"]:
            _add_reason(reasons, f"evaluated coverage is below {policy['min_evaluated_coverage']}")
        if oracle_agreement_rate is None or oracle_agreement_rate < policy["min_oracle_agreement_rate"]:
            _add_reason(reasons, f"oracle agreement is below {policy['min_oracle_agreement_rate']}")
        if row["mean_regret"] is None or float(row["mean_regret"]) > policy["max_mean_regret"]:
            _add_reason(reasons, f"mean regret exceeds {policy['max_mean_regret']}")
        if abstention_rate > policy["max_abstention_rate"]:
            _add_reason(reasons, f"abstention rate exceeds {policy['max_abstention_rate']}")
        if selected_reward_missing_rate > policy["max_selected_reward_missing_rate"]:
            _add_reason(reasons, f"selected reward missing rate exceeds {policy['max_selected_reward_missing_rate']}")
        if no_eligible_model_rate > policy["max_no_eligible_model_rate"]:
            _add_reason(reasons, f"no eligible model rate exceeds {policy['max_no_eligible_model_rate']}")
        if no_counterfactual_reward_rate > policy["max_no_counterfactual_reward_rate"]:
            _add_reason(reasons, f"no counterfactual reward rate exceeds {policy['max_no_counterfactual_reward_rate']}")
    decision = "not_required" if not has_cases and not policy["require_all_domains"] else ("admit" if not reasons else "hold")
    return {
        "schema": AUTONOMOUS_SELECTION_PROMOTION_DOMAIN_SCHEMA,
        "domain": row["domain"],
        "case_count": case_count,
        "evaluated_count": evaluated_count,
        "evaluated_coverage": evaluated_coverage,
        "oracle_agreement_count": int(row["oracle_agreement_count"]),
        "oracle_agreement_rate": oracle_agreement_rate,
        "mean_regret": row["mean_regret"],
        "abstention_rate": abstention_rate,
        "selected_reward_missing_rate": selected_reward_missing_rate,
        "no_eligible_model_rate": no_eligible_model_rate,
        "no_counterfactual_reward_rate": no_counterfactual_reward_rate,
        "decision": decision,
        "reasons": reasons,
    }


def evaluate_autonomous_selection_promotion(
    report: Mapping[str, Any],
    *,
    require_all_domains: bool = True,
    min_cases_per_domain: int = 1,
    min_evaluated_cases_per_domain: int = 1,
    min_evaluated_coverage: float = 0.5,
    min_oracle_agreement_rate: float = 0.5,
    max_mean_regret: float = 0.25,
    max_abstention_rate: float = 0.25,
    max_selected_reward_missing_rate: float = 0.0,
    max_no_eligible_model_rate: float = 0.0,
    max_no_counterfactual_reward_rate: float = 0.0,
) -> dict[str, Any]:
    """Admit or hold a validated replay policy using bounded cross-domain thresholds."""

    validated_report = validate_autonomous_selection_lab_report(report)
    policy = _policy_projection(
        require_all_domains=require_all_domains,
        min_cases_per_domain=min_cases_per_domain,
        min_evaluated_cases_per_domain=min_evaluated_cases_per_domain,
        min_evaluated_coverage=min_evaluated_coverage,
        min_oracle_agreement_rate=min_oracle_agreement_rate,
        max_mean_regret=max_mean_regret,
        max_abstention_rate=max_abstention_rate,
        max_selected_reward_missing_rate=max_selected_reward_missing_rate,
        max_no_eligible_model_rate=max_no_eligible_model_rate,
        max_no_counterfactual_reward_rate=max_no_counterfactual_reward_rate,
    )
    reasons: list[str] = []
    if validated_report["status"] != "completed":
        _add_reason(reasons, "selection replay report is not complete")
    if validated_report["case_count"] == 0:
        _add_reason(reasons, "selection replay contains no cases")
    domains = [_evaluate_domain(row, policy) for row in validated_report["domains"]]
    decision = "admit" if not reasons and all(row["decision"] != "hold" for row in domains) else "hold"
    body = {
        "schema": AUTONOMOUS_SELECTION_PROMOTION_SCHEMA,
        "source_report_digest": validated_report["report_digest"],
        "policy": policy,
        "decision": decision,
        "reasons": reasons,
        "domains": domains,
        "execution": _EXECUTION,
        "retention": _RETENTION,
        "secret_material": _SECRET_MATERIAL,
    }
    result = {**body, "promotion_digest": content_digest(body)}
    if len(canonical_json(result).encode("utf-8")) > MAX_AUTONOMOUS_SELECTION_PROMOTION_BYTES:
        _fail(f"report exceeds {MAX_AUTONOMOUS_SELECTION_PROMOTION_BYTES} bytes")
    return validate_autonomous_selection_promotion_report(result)


__all__ = [
    "AUTONOMOUS_SELECTION_PROMOTION_POLICY_SCHEMA",
    "AUTONOMOUS_SELECTION_PROMOTION_DOMAIN_SCHEMA",
    "AUTONOMOUS_SELECTION_PROMOTION_SCHEMA",
    "MAX_AUTONOMOUS_SELECTION_PROMOTION_REASONS",
    "MAX_AUTONOMOUS_SELECTION_PROMOTION_BYTES",
    "evaluate_autonomous_selection_promotion",
    "validate_autonomous_selection_promotion_report",
]
