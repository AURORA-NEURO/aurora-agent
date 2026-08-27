"""Provider-free model-selection replay and decision-quality measurement.

This module deliberately sits below provider invocation.  It accepts caller-owned model
metadata, health projections, and counterfactual evaluator rewards, then measures a selection
policy without requesting a credential, contacting a provider, or retaining task/candidate
payloads in the returned report.  The contract mirrors the TypeScript selection lab so a CI or
offline evaluator can exercise either SDK with the same case shape.
"""

from __future__ import annotations

from copy import deepcopy
import math
from typing import Any, Callable, Mapping, Sequence, TypedDict

from .authoring import canonical_json, content_digest
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError


AUTONOMOUS_SELECTION_LAB_CASE_SCHEMA = "bioprism-python-autonomous-selection-lab-case/0.1"
AUTONOMOUS_SELECTION_LAB_REPORT_SCHEMA = "bioprism-python-autonomous-selection-lab-report/0.1"
MAX_AUTONOMOUS_SELECTION_LAB_CASES = 4_096
MAX_AUTONOMOUS_SELECTION_LAB_CANDIDATES = 128
MAX_AUTONOMOUS_SELECTION_LAB_CAPABILITIES = 64
MAX_AUTONOMOUS_SELECTION_LAB_HEALTH_ROWS = 512
MAX_AUTONOMOUS_SELECTION_LAB_TASK_BYTES = 1_000_000
MAX_AUTONOMOUS_SELECTION_LAB_REPORT_BYTES = 2_000_000
MAX_AUTONOMOUS_SELECTION_LAB_OBSERVATIONS = 512

AUTONOMOUS_SELECTION_WEIGHTS_SCHEMA = "bioprism-autonomous-selection-weights/0.1"
DEFAULT_AUTONOMOUS_SELECTION_WEIGHTS: dict[str, float] = {
    "quality": 0.55,
    "reliability": 0.25,
    "cost": 0.10,
    "latency": 0.10,
    "exploration": 0.15,
}


class AutonomousSelectionWeights(TypedDict):
    """Cross-runtime multi-objective model-selection policy."""

    quality: float
    reliability: float
    cost: float
    latency: float
    exploration: float


_SELECTION_WEIGHT_NAMES = tuple(DEFAULT_AUTONOMOUS_SELECTION_WEIGHTS)


def normalize_autonomous_selection_weights(
    value: Mapping[str, Any] | None = None,
) -> AutonomousSelectionWeights:
    """Validate and fill the policy consumed by the Rust and TypeScript rankers.

    Weights are non-negative utility coefficients rather than probabilities.  Keeping the
    coefficients unnormalised preserves the Rust kernel's established contract, while refusing
    an all-zero policy prevents a decision from degenerating into lexical tie-breaking.
    """

    if value is not None and not isinstance(value, Mapping):
        _fail("selection weights must be an object")
    if value is not None:
        if any(not isinstance(key, str) for key in value):
            _fail("selection weights fields must be strings")
        unsupported = sorted(set(value).difference(_SELECTION_WEIGHT_NAMES))
        if unsupported:
            _fail("selection weights contain unsupported fields: " + ", ".join(unsupported))
    normalized: dict[str, float] = dict(DEFAULT_AUTONOMOUS_SELECTION_WEIGHTS)
    for name in _SELECTION_WEIGHT_NAMES:
        if value is None or name not in value:
            continue
        raw = value[name]
        _bounded_number(f"selection weights.{name}", raw, 0, 100)
        normalized[name] = round(float(raw), 12)
    if all(weight == 0 for weight in normalized.values()):
        _fail("selection weights must contain at least one positive value")
    return normalized  # type: ignore[return-value]

_RETENTION = "metadata_only;tasks_candidates_and_raw_selector_output_not_retained"
_SECRET_MATERIAL = "never_returned"
_STATUSES = (
    "evaluated",
    "abstained",
    "selected_reward_missing",
    "no_eligible_model",
    "no_counterfactual_reward",
)
_DOMAINS = tuple(AUTONOMOUS_DOMAIN_NAMES)

SelectionLabSelector = Callable[[Mapping[str, Any]], Mapping[str, Any]]


def _fail(message: str) -> "NoReturn":
    raise ArgumentError(f"autonomous selection lab {message}")


def _bounded_text(name: str, value: Any, maximum_bytes: int) -> str:
    if not isinstance(value, str) or not value.strip():
        _fail(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > maximum_bytes:
        _fail(f"{name} exceeds {maximum_bytes} bytes")
    if "\x00" in value:
        _fail(f"{name} contains a NUL character")
    return value


def _bounded_number(name: str, value: Any, minimum: float, maximum: float) -> float | int:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        _fail(f"{name} is outside its numeric bounds")
    if not math.isfinite(float(value)) or not minimum <= float(value) <= maximum:
        _fail(f"{name} is outside its numeric bounds")
    return value


def _bounded_integer(name: str, value: Any, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        _fail(f"{name} is outside its integer bounds")
    return value


def _optional_number(name: str, value: Any, minimum: float, maximum: float) -> None:
    if value is not None:
        _bounded_number(name, value, minimum, maximum)


def _optional_boolean(name: str, value: Any) -> None:
    if value is not None and not isinstance(value, bool):
        _fail(f"{name} must be boolean")


def _domain(value: Any) -> str:
    if not isinstance(value, str) or value not in _DOMAINS:
        _fail("domain is not a supported autonomous domain")
    return value


def _validate_capabilities(name: str, value: Any, *, required: bool = False) -> list[str]:
    if value is None and not required:
        return []
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        _fail(f"{name} must be a sequence")
    if len(value) > MAX_AUTONOMOUS_SELECTION_LAB_CAPABILITIES:
        _fail(f"{name} contains too many capabilities")
    result: list[str] = []
    for index, capability in enumerate(value):
        item = _bounded_text(f"{name}[{index}]", capability, 128)
        if item in result:
            _fail(f"{name} contains a duplicate capability")
        result.append(item)
    return result


def _validate_health_row(name: str, value: Any) -> None:
    if not isinstance(value, Mapping):
        _fail(f"{name} must be an object")
    if value.get("provider") is not None:
        _bounded_text(f"{name}.provider", value["provider"], 128)
    if value.get("circuit") is not None and value["circuit"] not in {"closed", "open"}:
        _fail(f"{name}.circuit is invalid")
    for field in ("consecutive_failures", "attempts", "successes", "failures", "quality_observations"):
        if field in value and value[field] is not None:
            _bounded_integer(f"{name}.{field}", value[field], 0, 100_000_000)
    for field in ("success_rate", "quality_mean"):
        if field in value and value[field] is not None:
            _bounded_number(f"{name}.{field}", value[field], 0, 1)
    for field in ("mean_latency_ms", "last_latency_ms"):
        if field in value and value[field] is not None:
            _bounded_number(f"{name}.{field}", value[field], 0, 86_400_000)
    if value.get("last_model") is not None:
        _bounded_text(f"{name}.last_model", value["last_model"], 512)
    if value.get("last_status_code") is not None:
        _bounded_integer(f"{name}.last_status_code", value["last_status_code"], 100, 999)
    for field in ("credential_required", "credential_ready", "registered", "eligible"):
        if field in value:
            _optional_boolean(f"{name}.{field}", value[field])
    if value.get("structured_output_mode") not in (None, "disabled", "json_object", "json_schema"):
        _fail(f"{name}.structured_output_mode is invalid")


def _validate_health_map(name: str, value: Any) -> None:
    if not isinstance(value, Mapping):
        _fail(f"{name} must be an object")
    if len(value) > MAX_AUTONOMOUS_SELECTION_LAB_HEALTH_ROWS:
        _fail(f"{name} contains too many rows")
    for key, row in value.items():
        _bounded_text(f"{name} key", key, 768)
        _validate_health_row(f"{name}.{key}", row)


def _validate_observations(value: Any) -> list[dict[str, Any]]:
    """Validate caller-owned online-learning evidence without accepting hidden state."""

    if value is None:
        return []
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        _fail("request.observations must be a sequence")
    if len(value) > MAX_AUTONOMOUS_SELECTION_LAB_OBSERVATIONS:
        _fail(
            "request.observations must contain at most "
            f"{MAX_AUTONOMOUS_SELECTION_LAB_OBSERVATIONS} items"
        )
    seen: set[str] = set()
    normalized: list[dict[str, Any]] = []
    for index, raw in enumerate(value):
        if not isinstance(raw, Mapping):
            _fail(f"request.observations[{index}] must be an object")
        arm_id = _bounded_text(
            f"request.observations[{index}].arm_id", raw.get("arm_id"), 768
        )
        if arm_id in seen:
            _fail(f"request.observations contains duplicate arm {arm_id}")
        seen.add(arm_id)
        pulls = _bounded_integer(
            f"request.observations[{index}].pulls", raw.get("pulls"), 0, 1_000_000_000
        )
        reward_sum = _bounded_number(
            f"request.observations[{index}].reward_sum",
            raw.get("reward_sum"),
            -1e12,
            1e12,
        )
        failures = _bounded_integer(
            f"request.observations[{index}].failures", raw.get("failures"), 0, pulls
        )
        disabled = raw.get("disabled")
        if "disabled" in raw:
            _optional_boolean(f"request.observations[{index}].disabled", disabled)
            if disabled is None:
                _fail(f"request.observations[{index}].disabled must be boolean")
        normalized.append(
            {
                "arm_id": arm_id,
                "pulls": pulls,
                "reward_sum": round(float(reward_sum), 12),
                "failures": failures,
                **({} if "disabled" not in raw else {"disabled": disabled}),
            }
        )
    return normalized


def _validate_candidate(value: Any, index: int, seen: set[str]) -> str:
    if not isinstance(value, Mapping):
        _fail(f"candidate {index} must be an object")
    provider = _bounded_text(f"candidate {index}.provider", value.get("provider"), 128)
    model = _bounded_text(f"candidate {index}.model", value.get("model"), 512)
    _validate_capabilities(f"candidate {index}.capabilities", value.get("capabilities"))
    _bounded_integer(f"candidate {index}.context_window_tokens", value.get("context_window_tokens"), 1, 1_000_000_000)
    _bounded_integer(f"candidate {index}.max_output_tokens", value.get("max_output_tokens"), 1, 1_000_000_000)
    _bounded_number(f"candidate {index}.quality", value.get("quality"), 0, 1)
    _bounded_number(f"candidate {index}.latency_ms", value.get("latency_ms"), 0, 86_400_000)
    _bounded_number(f"candidate {index}.cost_per_million_tokens", value.get("cost_per_million_tokens"), 0, 1_000_000_000)
    _bounded_number(f"candidate {index}.reliability", value.get("reliability"), 0, 1)
    _optional_boolean(f"candidate {index}.requires_credential", value.get("requires_credential"))
    _optional_boolean(f"candidate {index}.enabled", value.get("enabled"))
    arm_id = f"{provider}/{model}"
    if arm_id in seen:
        _fail(f"candidate arm {arm_id} is duplicated")
    seen.add(arm_id)
    return arm_id


def _validate_request(value: Any, expected_domain: str) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        _fail("case request must be an object")
    domain = _domain(value.get("domain"))
    if domain != expected_domain:
        _fail(f"case request domain must equal {expected_domain}")
    _bounded_text("request.task", value.get("task"), MAX_AUTONOMOUS_SELECTION_LAB_TASK_BYTES)
    _bounded_text("request.capability", value.get("capability"), 256)
    _bounded_text("request.risk_class", value.get("risk_class"), 128)
    if value.get("task_family") is not None:
        _bounded_text("request.task_family", value["task_family"], 256)
    context_digest = value.get("context_digest")
    if context_digest is not None and (not isinstance(context_digest, str) or len(context_digest) != 64 or any(character not in "0123456789abcdef" for character in context_digest)):
        _fail("request.context_digest must be a lowercase SHA-256 digest")
    _validate_capabilities("request.required_capabilities", value.get("required_capabilities"), required=True)
    _bounded_integer("request.estimated_input_tokens", value.get("estimated_input_tokens"), 0, 1_000_000_000)
    _bounded_integer("request.requested_output_tokens", value.get("requested_output_tokens"), 0, 1_000_000_000)
    for field, maximum in (("max_cost_per_million_tokens", 1_000_000_000), ("max_latency_ms", 86_400_000), ("min_quality", 1), ("min_selection_confidence", 1)):
        _optional_number(f"request.{field}", value.get(field), 0, maximum)
    _optional_boolean("request.require_json", value.get("require_json"))
    weights = normalize_autonomous_selection_weights(value.get("weights"))
    observations = _validate_observations(value.get("observations"))
    candidates = value.get("candidates")
    if not isinstance(candidates, Sequence) or isinstance(candidates, (str, bytes, bytearray)) or not 1 <= len(candidates) <= MAX_AUTONOMOUS_SELECTION_LAB_CANDIDATES:
        _fail(f"request.candidates must contain 1-{MAX_AUTONOMOUS_SELECTION_LAB_CANDIDATES} candidates")
    seen: set[str] = set()
    for index, candidate in enumerate(candidates):
        _validate_candidate(candidate, index, seen)
    _validate_health_map("request.provider_health", value.get("provider_health"))
    _validate_health_map("request.model_health", value.get("model_health"))
    return {**dict(value), "weights": weights, "observations": observations}


def _validate_case(value: Any, index: int, seen: set[str]) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        _fail(f"case {index} must be an object")
    case_id = _bounded_text(f"case {index}.case_id", value.get("case_id"), 256)
    if case_id in seen:
        _fail(f"case_id {case_id} is duplicated")
    seen.add(case_id)
    domain = _domain(value.get("domain"))
    request = _validate_request(value.get("request"), domain)
    rewards = value.get("rewards")
    if not isinstance(rewards, Mapping):
        _fail(f"case {index}.rewards must be an object")
    candidate_ids = {f"{candidate['provider']}/{candidate['model']}" for candidate in request["candidates"]}
    for arm_id, reward in rewards.items():
        if arm_id not in candidate_ids:
            _fail(f"case {index}.rewards contains an unknown arm")
        if reward is not None:
            _bounded_number(f"case {index}.rewards.{arm_id}", reward, -1, 1)
    return {**dict(value), "request": request, "rewards": dict(rewards)}


def rank_autonomous_models(request: Mapping[str, Any]) -> list[dict[str, Any]]:
    """Return the provider-free deterministic health/utility ranking used by the lab."""

    normalized = _validate_request(request, _domain(request.get("domain")))
    rankings: list[dict[str, Any]] = []
    provider_health = normalized["provider_health"]
    model_health = normalized["model_health"]
    weights = normalized["weights"]
    observations = {row["arm_id"]: row for row in normalized["observations"]}
    max_cost = max(
        1.0,
        *(float(candidate["cost_per_million_tokens"]) for candidate in normalized["candidates"]),
    )
    effective_metrics: dict[str, tuple[float, float]] = {}
    for candidate in normalized["candidates"]:
        arm_id = f"{candidate['provider']}/{candidate['model']}"
        health = model_health.get(arm_id)
        attempts = health.get("attempts", 0) if isinstance(health, Mapping) else 0
        if (
            isinstance(health, Mapping)
            and isinstance(health.get("success_rate"), (int, float))
            and not isinstance(health.get("success_rate"), bool)
            and attempts > 0
        ):
            confidence = min(float(attempts) / 12.0, 0.75)
            reliability = (1.0 - confidence) * float(candidate["reliability"]) + confidence * float(health["success_rate"])
            observed_latency = health.get("last_latency_ms")
            if observed_latency is None:
                observed_latency = health.get("mean_latency_ms")
            latency = float(candidate["latency_ms"])
            if isinstance(observed_latency, (int, float)) and not isinstance(observed_latency, bool):
                latency = (1.0 - confidence) * latency + confidence * float(observed_latency)
            effective_metrics[arm_id] = (reliability, latency)
        else:
            effective_metrics[arm_id] = (float(candidate["reliability"]), float(candidate["latency_ms"]))
    max_latency = max(1.0, *(latency for _, latency in effective_metrics.values()))
    total_pulls = sum(int(row["pulls"]) for row in observations.values())
    log_total = math.log(total_pulls + 1)
    for candidate in normalized["candidates"]:
        provider_name = candidate["provider"]
        model_name = candidate["model"]
        reasons: list[str] = []
        provider = provider_health.get(provider_name)
        arm_id = f"{provider_name}/{model_name}"
        model = model_health.get(arm_id)
        metrics = effective_metrics[arm_id]
        observation = observations.get(arm_id)
        if candidate.get("enabled") is False:
            reasons.append("candidate disabled")
        if not isinstance(provider, Mapping):
            reasons.append("provider not registered")
        if isinstance(provider, Mapping) and provider.get("circuit") == "open":
            reasons.append("provider circuit open")
        if isinstance(model, Mapping) and model.get("circuit") == "open":
            reasons.append("model circuit open")
        if not isinstance(provider, Mapping) or provider.get("credential_required") is not False or provider.get("credential_ready") is not True:
            reasons.append("credential not ready")
        if isinstance(provider, Mapping) and provider.get("registered") is False:
            reasons.append("provider not registered")
        if isinstance(provider, Mapping) and provider.get("eligible") is False:
            reasons.append("provider health ineligible")
        if isinstance(observation, Mapping) and observation.get("disabled") is True:
            reasons.append("disabled by learning policy")
        if candidate["max_output_tokens"] < normalized["requested_output_tokens"]:
            reasons.append("model output capacity is below the request")
        if candidate["context_window_tokens"] < normalized["estimated_input_tokens"] + normalized["requested_output_tokens"]:
            reasons.append("model context capacity is below the request")
        if any(required not in candidate.get("capabilities", []) for required in normalized["required_capabilities"]):
            reasons.append("model lacks a required capability")
        if normalized.get("require_json") is True and "structured_output" not in candidate.get("capabilities", []):
            reasons.append("model lacks structured output capability")
        if normalized.get("require_json") is True and isinstance(provider, Mapping) and provider.get("structured_output_mode") is None:
            reasons.append("provider structured output capability is unknown")
        if normalized.get("require_json") is True and isinstance(provider, Mapping) and provider.get("structured_output_mode") == "disabled":
            reasons.append("provider structured output is disabled")
        if normalized.get("max_cost_per_million_tokens") is not None and candidate["cost_per_million_tokens"] > normalized["max_cost_per_million_tokens"]:
            reasons.append("model cost exceeds the caller budget")
        if normalized.get("max_latency_ms") is not None and metrics[1] > normalized["max_latency_ms"]:
            reasons.append("model latency exceeds the caller bound")
        if normalized.get("min_quality") is not None and candidate["quality"] < normalized["min_quality"]:
            reasons.append("model quality is below the caller floor")
        pulls = 0 if observation is None else int(observation["pulls"])
        mean_reward = 0.0 if pulls == 0 else float(observation["reward_sum"]) / pulls
        exploration_bonus = (
            float(weights["exploration"])
            if pulls == 0
            else float(weights["exploration"]) * math.sqrt(log_total / pulls)
        )
        base_score = (
            float(weights["quality"]) * float(candidate["quality"])
            + float(weights["reliability"]) * metrics[0]
            + float(weights["exploration"]) * mean_reward
            - float(weights["cost"]) * (float(candidate["cost_per_million_tokens"]) / max_cost)
            - float(weights["latency"]) * (metrics[1] / max_latency)
        )
        rankings.append(
            {
                "provider": provider_name,
                "model": model_name,
                "score": round(base_score + exploration_bonus, 12),
                "eligible": not reasons,
                "reasons": reasons,
                "base_score": round(base_score, 12),
                "exploration_bonus": round(exploration_bonus, 12),
                "observed_pulls": pulls,
            }
        )
    rankings.sort(key=lambda row: (-int(row["eligible"]), -row["score"], row["provider"], row["model"]))
    return rankings


def autonomous_selection_confidence(ranking: Sequence[Mapping[str, Any]]) -> float:
    """Return normalized top-versus-runner-up separation, never answer correctness."""

    eligible = sorted((row for row in ranking if row.get("eligible") is True), key=lambda row: (-float(row["score"]), str(row["provider"]), str(row["model"])))
    if not eligible:
        return 0.0
    if len(eligible) == 1:
        return 1.0
    top, runner_up = eligible[0], eligible[1]
    return max(0.0, min(1.0, (float(top["score"]) - float(runner_up["score"])) / (1 + abs(float(top["score"])) + abs(float(runner_up["score"])))))


def _default_decision(request: Mapping[str, Any], ranking: Sequence[Mapping[str, Any]]) -> dict[str, Any]:
    top = next((row for row in ranking if row["eligible"]), None)
    return {
        "selected_model": None if top is None else {"provider": top["provider"], "model": top["model"]},
        "strategy": "deterministic_health_utility",
        "ranking": [dict(row) for row in ranking],
        "abstention_reason": None if top is not None else "no eligible model",
        "selection_confidence": autonomous_selection_confidence(ranking),
        "min_selection_confidence": request.get("min_selection_confidence"),
    }


def _validate_decision(value: Any, ranking: Sequence[Mapping[str, Any]], request: Mapping[str, Any]) -> tuple[str | None, float | None]:
    if not isinstance(value, Mapping):
        _fail("selector returned a non-object decision")
    if "selected_model" not in value:
        _fail("selector decision omitted selected_model")
    selected = value["selected_model"]
    selected_id: str | None = None
    if selected is not None:
        if not isinstance(selected, Mapping):
            _fail("selector selected_model must be null or an object")
        provider = _bounded_text("selector selected_model.provider", selected.get("provider"), 128)
        model = _bounded_text("selector selected_model.model", selected.get("model"), 512)
        selected_id = f"{provider}/{model}"
        candidate_ids = {f"{candidate['provider']}/{candidate['model']}" for candidate in request["candidates"]}
        if selected_id not in candidate_ids:
            _fail("selector selected an unknown model arm")
        canonical = next((row for row in ranking if f"{row['provider']}/{row['model']}" == selected_id), None)
        if canonical is None or canonical["eligible"] is not True:
            _fail("selector selected an ineligible model arm")
    if value.get("strategy") is not None:
        _bounded_text("selector strategy", value["strategy"], 128)
    if value.get("abstention_reason") is not None:
        _bounded_text("selector abstention_reason", value["abstention_reason"], 4_096)
    confidence = value.get("selection_confidence")
    if confidence is not None:
        _bounded_number("selector selection_confidence", confidence, 0, 1)
        confidence = float(confidence)
    return selected_id, confidence


def _selection_digest(result: Mapping[str, Any]) -> str:
    return content_digest({
        "case_id": result["case_id"],
        "selected_model_id": result["selected_model_id"],
        "oracle_model_id": result["oracle_model_id"],
        "selected_reward": result["selected_reward"],
        "oracle_reward": result["oracle_reward"],
        "regret": result["regret"],
        "status": result["status"],
        "selection_confidence": result["selection_confidence"],
    })


def _rounded(value: float) -> float:
    return round(float(value), 12)


def _validate_case_result(value: Any, index: int) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        _fail(f"report case {index} must be an object")
    if value.get("schema") != AUTONOMOUS_SELECTION_LAB_CASE_SCHEMA:
        _fail(f"report case {index} schema is invalid")
    case_id = _bounded_text(f"report case {index}.case_id", value.get("case_id"), 256)
    domain = _domain(value.get("domain"))
    for field in ("task_digest", "request_digest", "selection_digest"):
        digest = value.get(field)
        if not isinstance(digest, str) or len(digest) != 64 or any(character not in "0123456789abcdef" for character in digest):
            _fail(f"report case {case_id}.{field} is malformed")
    for field in ("selected_model_id", "oracle_model_id"):
        if value.get(field) is not None:
            _bounded_text(f"report case {case_id}.{field}", value[field], 768)
    for field in ("selected_reward", "oracle_reward"):
        if value.get(field) is not None:
            _bounded_number(f"report case {case_id}.{field}", value[field], -1, 1)
    if value.get("regret") is not None:
        _bounded_number(f"report case {case_id}.regret", value["regret"], 0, 2)
    if value.get("selection_confidence") is not None:
        _bounded_number(f"report case {case_id}.selection_confidence", value["selection_confidence"], 0, 1)
    _bounded_integer(f"report case {case_id}.eligible_candidate_count", value.get("eligible_candidate_count"), 0, MAX_AUTONOMOUS_SELECTION_LAB_CANDIDATES)
    _bounded_integer(f"report case {case_id}.counterfactual_candidate_count", value.get("counterfactual_candidate_count"), 0, MAX_AUTONOMOUS_SELECTION_LAB_CANDIDATES)
    if value.get("status") not in _STATUSES:
        _fail(f"report case {case_id}.status is invalid")
    if value.get("retention") != _RETENTION or value.get("secret_material") != _SECRET_MATERIAL:
        _fail(f"report case {case_id} retention posture is invalid")
    if _selection_digest(value) != value["selection_digest"]:
        _fail(f"report case {case_id} selection digest is invalid")
    return dict(value)


def _validate_domain_report(value: Any, index: int) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        _fail(f"report domain {index} must be an object")
    domain = _domain(value.get("domain"))
    for field in ("case_count", "evaluated_count", "abstained_count", "selected_reward_missing_count", "no_eligible_model_count", "no_counterfactual_reward_count", "oracle_agreement_count"):
        _bounded_integer(f"report domain {domain}.{field}", value.get(field), 0, MAX_AUTONOMOUS_SELECTION_LAB_CASES)
    _bounded_number(f"report domain {domain}.total_regret", value.get("total_regret"), 0, MAX_AUTONOMOUS_SELECTION_LAB_CASES * 2)
    for field, minimum, maximum in (("mean_selected_reward", -1, 1), ("mean_oracle_reward", -1, 1), ("mean_regret", 0, 2)):
        if value.get(field) is not None:
            _bounded_number(f"report domain {domain}.{field}", value[field], minimum, maximum)
    _bounded_number(f"report domain {domain}.evaluated_coverage", value.get("evaluated_coverage"), 0, 1)
    return dict(value)


def validate_autonomous_selection_lab_report(value: Mapping[str, Any]) -> dict[str, Any]:
    """Validate, semantically cross-check, and deep-copy a selection-lab report."""

    if not isinstance(value, Mapping):
        _fail("report must be an object")
    try:
        encoded = canonical_json(value).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise ArgumentError("autonomous selection lab report is not canonical JSON") from error
    if len(encoded) > MAX_AUTONOMOUS_SELECTION_LAB_REPORT_BYTES:
        _fail(f"report exceeds {MAX_AUTONOMOUS_SELECTION_LAB_REPORT_BYTES} bytes")
    if value.get("schema") != AUTONOMOUS_SELECTION_LAB_REPORT_SCHEMA:
        _fail("report schema is invalid")
    if value.get("status") not in {"completed", "insufficient_coverage"}:
        _fail("report status is invalid")
    _bounded_text("report selector_label", value.get("selector_label"), 256)
    if not isinstance(value.get("require_all_domains"), bool):
        _fail("report require_all_domains must be boolean")
    for field in ("case_count", "evaluated_case_count", "abstained_case_count", "selected_reward_missing_count", "no_eligible_model_count", "no_counterfactual_reward_count", "oracle_agreement_count"):
        _bounded_integer(f"report {field}", value.get(field), 0, MAX_AUTONOMOUS_SELECTION_LAB_CASES)
    _bounded_number("report total_regret", value.get("total_regret"), 0, MAX_AUTONOMOUS_SELECTION_LAB_CASES * 2)
    if value.get("oracle_agreement_rate") is not None:
        _bounded_number("report oracle_agreement_rate", value["oracle_agreement_rate"], 0, 1)
    if value.get("mean_regret") is not None:
        _bounded_number("report mean_regret", value["mean_regret"], 0, 2)
    domains_raw = value.get("domains")
    cases_raw = value.get("cases")
    missing_raw = value.get("missing_domains")
    if not isinstance(domains_raw, Sequence) or isinstance(domains_raw, (str, bytes, bytearray)) or len(domains_raw) != len(_DOMAINS):
        _fail("report domains are malformed")
    if not isinstance(cases_raw, Sequence) or isinstance(cases_raw, (str, bytes, bytearray)) or len(cases_raw) != value["case_count"]:
        _fail("report cases are malformed")
    if not isinstance(missing_raw, Sequence) or isinstance(missing_raw, (str, bytes, bytearray)):
        _fail("report missing_domains is malformed")
    domains = [_validate_domain_report(row, index) for index, row in enumerate(domains_raw)]
    cases = [_validate_case_result(row, index) for index, row in enumerate(cases_raw)]
    if tuple(row["domain"] for row in domains) != _DOMAINS:
        _fail("report domains are not in canonical order or contain duplicates")
    missing = [row["domain"] for row in domains if row["case_count"] == 0]
    if list(missing_raw) != missing:
        _fail("report missing_domains does not match domain coverage")
    expected_status = "insufficient_coverage" if value["require_all_domains"] and missing else "completed"
    if value["status"] != expected_status:
        _fail("report status does not match its coverage policy")
    per_domain: dict[str, dict[str, Any]] = {
        domain: {
            "case_count": 0,
            "evaluated_count": 0,
            "abstained_count": 0,
            "selected_reward_missing_count": 0,
            "no_eligible_model_count": 0,
            "no_counterfactual_reward_count": 0,
            "oracle_agreement_count": 0,
            "total_regret": 0.0,
            "selected_rewards": [],
            "oracle_rewards": [],
            "regrets": [],
        }
        for domain in _DOMAINS
    }
    case_ids: set[str] = set()
    for result in cases:
        if result["case_id"] in case_ids:
            _fail("report case ids are duplicated")
        case_ids.add(result["case_id"])
        aggregate = per_domain[result["domain"]]
        aggregate["case_count"] += 1
        status = result["status"]
        if status == "evaluated":
            if any(result[field] is None for field in ("selected_model_id", "oracle_model_id", "selected_reward", "oracle_reward", "regret")):
                _fail(f"report case {result['case_id']} evaluated fields are incomplete")
            expected_regret = _rounded(max(0.0, float(result["oracle_reward"]) - float(result["selected_reward"])))
            if result["regret"] != expected_regret:
                _fail(f"report case {result['case_id']} regret is inconsistent with its rewards")
            aggregate["evaluated_count"] += 1
            aggregate["selected_rewards"].append(float(result["selected_reward"]))
            aggregate["oracle_rewards"].append(float(result["oracle_reward"]))
            aggregate["regrets"].append(float(result["regret"]))
            aggregate["total_regret"] += float(result["regret"])
            if result["selected_model_id"] == result["oracle_model_id"]:
                aggregate["oracle_agreement_count"] += 1
        elif status == "abstained":
            if result["selected_model_id"] is not None or result["selected_reward"] is not None or result["oracle_model_id"] is None or result["oracle_reward"] is None or result["regret"] is not None:
                _fail(f"report case {result['case_id']} abstention fields are inconsistent")
            aggregate["abstained_count"] += 1
        elif status == "selected_reward_missing":
            if result["selected_model_id"] is None or result["selected_reward"] is not None or result["oracle_model_id"] is None or result["oracle_reward"] is None or result["regret"] is not None:
                _fail(f"report case {result['case_id']} missing-reward fields are inconsistent")
            aggregate["selected_reward_missing_count"] += 1
        elif status == "no_eligible_model":
            if any(result[field] not in (0, None) for field in ("eligible_candidate_count", "counterfactual_candidate_count", "selected_model_id", "oracle_model_id", "selected_reward", "oracle_reward", "regret")):
                _fail(f"report case {result['case_id']} no-eligible fields are inconsistent")
            aggregate["no_eligible_model_count"] += 1
        else:
            if result["counterfactual_candidate_count"] != 0 or result["oracle_model_id"] is not None or result["oracle_reward"] is not None or result["regret"] is not None:
                _fail(f"report case {result['case_id']} no-counterfactual fields are inconsistent")
            aggregate["no_counterfactual_reward_count"] += 1
    total = {field: 0 for field in ("evaluated_count", "abstained_count", "selected_reward_missing_count", "no_eligible_model_count", "no_counterfactual_reward_count", "oracle_agreement_count")}
    total_regret = 0.0
    for row in domains:
        aggregate = per_domain[row["domain"]]
        for field in ("case_count", "evaluated_count", "abstained_count", "selected_reward_missing_count", "no_eligible_model_count", "no_counterfactual_reward_count", "oracle_agreement_count"):
            if row[field] != aggregate[field]:
                _fail(f"report domain {row['domain']}.{field} disagrees with its cases")
        def mean(values: list[float]) -> float | None:
            return None if not values else _rounded(sum(values) / len(values))
        if (
            row["total_regret"] != _rounded(aggregate["total_regret"])
            or row["mean_selected_reward"] != mean(aggregate["selected_rewards"])
            or row["mean_oracle_reward"] != mean(aggregate["oracle_rewards"])
            or row["mean_regret"] != mean(aggregate["regrets"])
            or row["evaluated_coverage"] != (0 if aggregate["case_count"] == 0 else _rounded(aggregate["evaluated_count"] / aggregate["case_count"]))
        ):
            _fail(f"report domain {row['domain']} metrics disagree with its cases")
        for field in total:
            total[field] += aggregate[field]
        total_regret += aggregate["total_regret"]
    if (
        value["case_count"] != len(cases)
        or value["evaluated_case_count"] != total["evaluated_count"]
        or value["abstained_case_count"] != total["abstained_count"]
        or value["selected_reward_missing_count"] != total["selected_reward_missing_count"]
        or value["no_eligible_model_count"] != total["no_eligible_model_count"]
        or value["no_counterfactual_reward_count"] != total["no_counterfactual_reward_count"]
        or value["oracle_agreement_count"] != total["oracle_agreement_count"]
        or value["total_regret"] != _rounded(total_regret)
        or value["mean_regret"] != (None if total["evaluated_count"] == 0 else _rounded(total_regret / total["evaluated_count"]))
        or value["oracle_agreement_rate"] != (None if total["evaluated_count"] == 0 else _rounded(total["oracle_agreement_count"] / total["evaluated_count"]))
    ):
        _fail("report aggregate metrics disagree with its cases")
    report_digest = value.get("report_digest")
    if not isinstance(report_digest, str) or len(report_digest) != 64 or any(character not in "0123456789abcdef" for character in report_digest):
        _fail("report digest is malformed")
    body = {key: item for key, item in value.items() if key != "report_digest"}
    if content_digest(body) != report_digest:
        _fail("report digest does not match its canonical projection")
    if value.get("retention") != _RETENTION or value.get("secret_material") != _SECRET_MATERIAL:
        _fail("report retention posture is invalid")
    return deepcopy(dict(value))


def evaluate_autonomous_selection_policy(
    cases: Sequence[Mapping[str, Any]],
    *,
    selector: SelectionLabSelector | None = None,
    selector_label: str | None = None,
    require_all_domains: bool = False,
) -> dict[str, Any]:
    """Replay deterministic model selection against caller-owned rewards across all domains.

    ``selector`` receives a validated, value-only request and must return a decision containing
    ``selected_model``.  It is optional: the default is the same local health/utility ranker
    exposed by :func:`rank_autonomous_models`.  The lab never invokes a provider and does not
    mutate caller-owned selector or learning state.
    """

    if not isinstance(cases, Sequence) or isinstance(cases, (str, bytes, bytearray)):
        _fail("cases must be a sequence")
    if len(cases) > MAX_AUTONOMOUS_SELECTION_LAB_CASES:
        _fail(f"cases must contain at most {MAX_AUTONOMOUS_SELECTION_LAB_CASES} items")
    if selector is not None and not callable(selector):
        _fail("selector must be callable")
    if not isinstance(require_all_domains, bool):
        _fail("require_all_domains must be boolean")
    label = ("caller_selector" if selector is not None else "deterministic_health_utility") if selector_label is None else _bounded_text("selector_label", selector_label, 256)
    seen: set[str] = set()
    normalized_cases = sorted((_validate_case(value, index, seen) for index, value in enumerate(cases)), key=lambda value: value["case_id"])
    results: list[dict[str, Any]] = []
    for lab_case in normalized_cases:
        request = lab_case["request"]
        ranking = rank_autonomous_models(request)
        rewards = lab_case["rewards"]
        counterfactual = [row for row in ranking if row["eligible"] and isinstance(rewards.get(f"{row['provider']}/{row['model']}"), (int, float)) and not isinstance(rewards.get(f"{row['provider']}/{row['model']}"), bool)]
        counterfactual.sort(key=lambda row: (-float(rewards[f"{row['provider']}/{row['model']}"]), f"{row['provider']}/{row['model']}"))
        decision = _default_decision(request, ranking) if selector is None else selector(deepcopy(request))
        selected_id, confidence = _validate_decision(decision, ranking, request)
        selected_reward = rewards.get(selected_id) if selected_id is not None and isinstance(rewards.get(selected_id), (int, float)) and not isinstance(rewards.get(selected_id), bool) else None
        oracle = counterfactual[0] if counterfactual else None
        oracle_id = None if oracle is None else f"{oracle['provider']}/{oracle['model']}"
        oracle_reward = None if oracle_id is None else rewards[oracle_id]
        if not any(row["eligible"] for row in ranking):
            status = "no_eligible_model"
        elif not counterfactual:
            status = "no_counterfactual_reward"
        elif selected_id is None:
            status = "abstained"
        elif selected_reward is None:
            status = "selected_reward_missing"
        else:
            status = "evaluated"
        regret = None if status != "evaluated" else _rounded(max(0.0, float(oracle_reward) - float(selected_reward)))
        task_digest = content_digest(request["task"])
        request_digest = content_digest({**request, "task": task_digest})
        result = {
            "schema": AUTONOMOUS_SELECTION_LAB_CASE_SCHEMA,
            "case_id": lab_case["case_id"],
            "domain": lab_case["domain"],
            "task_digest": task_digest,
            "request_digest": request_digest,
            "selected_model_id": selected_id,
            "oracle_model_id": oracle_id,
            "selected_reward": selected_reward,
            "oracle_reward": oracle_reward,
            "regret": regret,
            "selection_confidence": confidence,
            "eligible_candidate_count": sum(1 for row in ranking if row["eligible"]),
            "counterfactual_candidate_count": len(counterfactual),
            "status": status,
            "selection_digest": "",
            "retention": _RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }
        result["selection_digest"] = _selection_digest(result)
        results.append(result)
    domain_reports: list[dict[str, Any]] = []
    for domain in _DOMAINS:
        domain_cases = [result for result in results if result["domain"] == domain]
        evaluated = [result for result in domain_cases if result["status"] == "evaluated"]
        mean = lambda values: None if not values else _rounded(sum(float(value) for value in values) / len(values))
        domain_reports.append({
            "domain": domain,
            "case_count": len(domain_cases),
            "evaluated_count": len(evaluated),
            "abstained_count": sum(result["status"] == "abstained" for result in domain_cases),
            "selected_reward_missing_count": sum(result["status"] == "selected_reward_missing" for result in domain_cases),
            "no_eligible_model_count": sum(result["status"] == "no_eligible_model" for result in domain_cases),
            "no_counterfactual_reward_count": sum(result["status"] == "no_counterfactual_reward" for result in domain_cases),
            "oracle_agreement_count": sum(result["selected_model_id"] == result["oracle_model_id"] for result in evaluated),
            "total_regret": _rounded(sum(float(result["regret"]) for result in evaluated)),
            "mean_selected_reward": mean([result["selected_reward"] for result in evaluated]),
            "mean_oracle_reward": mean([result["oracle_reward"] for result in evaluated]),
            "mean_regret": mean([result["regret"] for result in evaluated]),
            "evaluated_coverage": 0 if not domain_cases else _rounded(len(evaluated) / len(domain_cases)),
        })
    missing_domains = [row["domain"] for row in domain_reports if row["case_count"] == 0]
    evaluated = [result for result in results if result["status"] == "evaluated"]
    body = {
        "schema": AUTONOMOUS_SELECTION_LAB_REPORT_SCHEMA,
        "status": "insufficient_coverage" if require_all_domains and missing_domains else "completed",
        "selector_label": label,
        "require_all_domains": require_all_domains,
        "case_count": len(results),
        "evaluated_case_count": len(evaluated),
        "abstained_case_count": sum(result["status"] == "abstained" for result in results),
        "selected_reward_missing_count": sum(result["status"] == "selected_reward_missing" for result in results),
        "no_eligible_model_count": sum(result["status"] == "no_eligible_model" for result in results),
        "no_counterfactual_reward_count": sum(result["status"] == "no_counterfactual_reward" for result in results),
        "oracle_agreement_count": sum(result["selected_model_id"] == result["oracle_model_id"] for result in evaluated),
        "oracle_agreement_rate": None if not evaluated else _rounded(sum(result["selected_model_id"] == result["oracle_model_id"] for result in evaluated) / len(evaluated)),
        "total_regret": _rounded(sum(float(result["regret"]) for result in evaluated)),
        "mean_regret": None if not evaluated else _rounded(sum(float(result["regret"]) for result in evaluated) / len(evaluated)),
        "missing_domains": missing_domains,
        "domains": domain_reports,
        "cases": results,
        "retention": _RETENTION,
        "secret_material": _SECRET_MATERIAL,
    }
    report = {**body, "report_digest": content_digest(body)}
    if len(canonical_json(report).encode("utf-8")) > MAX_AUTONOMOUS_SELECTION_LAB_REPORT_BYTES:
        _fail(f"report exceeds {MAX_AUTONOMOUS_SELECTION_LAB_REPORT_BYTES} bytes")
    return validate_autonomous_selection_lab_report(report)


__all__ = [
    "AUTONOMOUS_SELECTION_LAB_CASE_SCHEMA",
    "AUTONOMOUS_SELECTION_LAB_REPORT_SCHEMA",
    "MAX_AUTONOMOUS_SELECTION_LAB_CASES",
    "MAX_AUTONOMOUS_SELECTION_LAB_CANDIDATES",
    "MAX_AUTONOMOUS_SELECTION_LAB_CAPABILITIES",
    "MAX_AUTONOMOUS_SELECTION_LAB_HEALTH_ROWS",
    "MAX_AUTONOMOUS_SELECTION_LAB_TASK_BYTES",
    "MAX_AUTONOMOUS_SELECTION_LAB_REPORT_BYTES",
    "MAX_AUTONOMOUS_SELECTION_LAB_OBSERVATIONS",
    "AUTONOMOUS_SELECTION_WEIGHTS_SCHEMA",
    "DEFAULT_AUTONOMOUS_SELECTION_WEIGHTS",
    "AutonomousSelectionWeights",
    "normalize_autonomous_selection_weights",
    "SelectionLabSelector",
    "rank_autonomous_models",
    "autonomous_selection_confidence",
    "evaluate_autonomous_selection_policy",
    "validate_autonomous_selection_lab_report",
]
