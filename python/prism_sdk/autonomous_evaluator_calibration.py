"""Provider-free evaluator calibration and holdout admission for the autonomous brain.

Calibration is deliberately a value-only boundary.  A caller supplies normalized domain
evidence and an independent binary label (for example, a reviewed pass/fail outcome).  The
module runs the reviewed evaluator adapters over deterministic calibration and holdout splits,
retains only aggregate metrics and digests, and emits an explicit learning-admission decision.
It never resolves credentials, calls a provider, stores evidence cases, or treats calibration as
proof of scientific truth.

The report is intentionally strict because it can gate model selection, bandit updates, and
portfolio admission.  A report is bound to the evaluator catalogue, case-set digests, split
policy, metric thresholds, and a canonical report digest.  Replaying the same caller-owned case
set therefore detects evaluator drift, case drift, and tampering before a learned policy is
enabled.
"""

from __future__ import annotations

from copy import deepcopy
from pathlib import Path
import json
import math
import sqlite3
import threading
from typing import Any, Mapping, Protocol, Sequence

from .authoring import canonical_json, content_digest
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError
from .evaluators import DomainEvaluatorAdapter, DomainEvaluatorRegistry


AUTONOMOUS_EVALUATOR_CALIBRATION_SCHEMA = (
    "bioprism-python-autonomous-evaluator-calibration/0.1"
)
AUTONOMOUS_EVALUATOR_CALIBRATION_REPLAY_SCHEMA = (
    "bioprism-python-autonomous-evaluator-calibration-replay/0.1"
)
AUTONOMOUS_EVALUATOR_CALIBRATION_ADMISSION_SCHEMA = (
    "bioprism-python-autonomous-evaluator-calibration-admission/0.1"
)
AUTONOMOUS_EVALUATOR_CALIBRATION_REGISTRY_SCHEMA = (
    "bioprism-python-autonomous-evaluator-calibration-registry/0.1"
)
AUTONOMOUS_EVALUATOR_CALIBRATION_SQLITE_SCHEMA = (
    "bioprism-python-autonomous-evaluator-calibration-sqlite/0.1"
)
MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_CASES = 2_048
MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_BINS = 20
MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_DOMAINS = len(AUTONOMOUS_DOMAIN_NAMES)
MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_REASON_COUNT = 64
MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_REPORT_BYTES = 512_000
MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_REGISTRY_REPORTS = 128
MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_REGISTRY_BYTES = 8_000_000
MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_CONTEXT_BYTES = 32_000
MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_SEED_BYTES = 256

_DOMAINS = tuple(AUTONOMOUS_DOMAIN_NAMES)
_RETENTION = "aggregate_metrics_and_digests_only;calibration_cases_and_labels_not_retained"
_SECRET_MATERIAL = "never_returned"
_SPLITS = {"calibration", "holdout"}
_DOMAIN_STATUSES = {"ready", "insufficient_calibration", "insufficient_holdout", "miscalibrated"}
_REPORT_STATUSES = {"ready", "insufficient_coverage", "insufficient_evidence", "miscalibrated"}
_ADMISSION_DECISIONS = {"admit_learning", "hold_learning"}
_SECRET_KEYS = {
    "apikey",
    "authorization",
    "bearer",
    "credential",
    "password",
    "privatekey",
    "refreshtoken",
    "secret",
    "token",
}


def _fail(message: str) -> "NoReturn":
    raise ArgumentError(f"autonomous evaluator calibration {message}")


def _bounded_text(name: str, value: Any, maximum: int) -> str:
    if not isinstance(value, str) or not value or "\x00" in value:
        _fail(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > maximum:
        _fail(f"{name} exceeds {maximum} bytes")
    return value


def _identifier(name: str, value: Any, maximum: int = 256) -> str:
    text = _bounded_text(name, value, maximum)
    allowed = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:/-"
    if any(character not in allowed for character in text):
        _fail(f"{name} contains an unsafe identifier character")
    return text


def _digest(name: str, value: Any, *, allow_none: bool = False) -> str | None:
    if value is None and allow_none:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(
        character not in "0123456789abcdef" for character in value
    ):
        _fail(f"{name} must be a lowercase SHA-256 digest")
    return value


def _bounded_integer(name: str, value: Any, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        _fail(f"{name} must be between {minimum} and {maximum}")
    return value


def _bounded_float(name: str, value: Any, minimum: float, maximum: float) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        _fail(f"{name} must be numeric")
    result = float(value)
    if not math.isfinite(result) or not minimum <= result <= maximum:
        _fail(f"{name} must be between {minimum} and {maximum}")
    return result


def _rounded(value: float) -> float:
    return round(float(value), 12)


def _sequence(name: str, value: Any, maximum: int) -> tuple[Any, ...]:
    if isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence):
        _fail(f"{name} must be an array")
    if len(value) > maximum:
        _fail(f"{name} contains more than {maximum} entries")
    return tuple(value)


def _safe_metadata(value: Any, *, path: str = "$") -> None:
    """Reject credential-shaped metadata before an adapter can see it."""

    if isinstance(value, Mapping):
        for raw_key, raw_value in value.items():
            if not isinstance(raw_key, str):
                _fail(f"{path} contains a non-string key")
            normalized = "".join(character for character in raw_key.lower() if character.isalnum())
            if normalized in _SECRET_KEYS or any(
                marker in normalized for marker in ("apikey", "authorization", "password", "privatekey", "refreshtoken")
            ):
                _fail(f"{path}.{raw_key} contains forbidden secret-shaped metadata")
            _safe_metadata(raw_value, path=f"{path}.{raw_key}")
    elif isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        for index, item in enumerate(value):
            _safe_metadata(item, path=f"{path}[{index}]")
    else:
        try:
            json.dumps(value, ensure_ascii=False, allow_nan=False)
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"autonomous evaluator calibration {path} is not JSON-safe") from error


def _domains(value: Sequence[str] | None) -> tuple[str, ...]:
    selected = _DOMAINS if value is None else _sequence("target domains", value, MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_DOMAINS)
    if not selected:
        _fail("target domains must not be empty")
    normalized: list[str] = []
    for domain in selected:
        if not isinstance(domain, str) or domain not in _DOMAINS:
            _fail("target domains contain an unsupported autonomous domain")
        if domain in normalized:
            _fail("target domains contain duplicates")
        normalized.append(domain)
    return tuple(normalized)


def _normalize_case(
    value: Any,
    index: int,
    *,
    registry: DomainEvaluatorRegistry,
    seed: str,
    holdout_fraction: float,
) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        _fail(f"case {index} must be an object")
    case_id = _identifier(f"case {index}.case_id", value.get("case_id"))
    domain = value.get("domain")
    if not isinstance(domain, str) or domain not in _DOMAINS:
        _fail(f"case {case_id}.domain is unsupported")
    raw_evidence = value.get("evidence")
    if not isinstance(raw_evidence, Mapping):
        _fail(f"case {case_id}.evidence must be an object")
    context = value.get("context", {"domain": domain})
    if context is None:
        context = {"domain": domain}
    if not isinstance(context, Mapping):
        _fail(f"case {case_id}.context must be an object")
    _safe_metadata(raw_evidence, path=f"case {case_id}.evidence")
    _safe_metadata(context, path=f"case {case_id}.context")
    label = value.get("label")
    if label is not None and (isinstance(label, bool) or label not in (0, 1)):
        _fail(f"case {case_id}.label must be 0, 1, or null")
    split = value.get("split")
    if split is not None and split not in _SPLITS:
        _fail(f"case {case_id}.split is invalid")
    try:
        adapter = registry.resolve_for_autonomous_domain(domain)
        evidence = adapter.normalize_evidence(raw_evidence).to_dict()
    except Exception as error:
        raise ArgumentError(f"autonomous evaluator calibration case {case_id} evidence was rejected") from error
    expected_id = value.get("expected_evaluator_id")
    expected_version = value.get("expected_evaluator_version")
    if expected_id is not None and _bounded_text(f"case {case_id}.expected_evaluator_id", expected_id, 256) != adapter.evaluator_id:
        _fail(f"case {case_id} expected evaluator id does not match the registry")
    if expected_version is not None and _bounded_text(f"case {case_id}.expected_evaluator_version", expected_version, 128) != adapter.evaluator_version:
        _fail(f"case {case_id} expected evaluator version does not match the registry")
    context_value = json.loads(canonical_json(dict(context)))
    evidence_digest = content_digest(evidence)
    split_value = split
    if split_value is None:
        split_digest = content_digest({
            "schema": AUTONOMOUS_EVALUATOR_CALIBRATION_SCHEMA,
            "case_id": case_id,
            "domain": domain,
            "seed": seed,
        })
        split_value = "holdout" if int(split_digest[:8], 16) / 0xFFFFFFFF < holdout_fraction else "calibration"
    descriptor = {
        "case_id": case_id,
        "domain": domain,
        "evidence_digest": evidence_digest,
        "context_digest": content_digest(context_value),
        "label": label,
        "split": split_value,
        "evaluator_id": adapter.evaluator_id,
        "evaluator_version": adapter.evaluator_version,
    }
    return {
        "case_id": case_id,
        "domain": domain,
        "evidence": evidence,
        "context": context_value,
        "label": label,
        "split": split_value,
        "evaluator_id": adapter.evaluator_id,
        "evaluator_version": adapter.evaluator_version,
        "case_digest": content_digest(descriptor),
    }


def _metrics(observations: Sequence[tuple[float, int | None]], *, bins: int, threshold: float) -> dict[str, Any]:
    total = len(observations)
    scored = [(score, label) for score, label in observations if label in (0, 1)]
    unscored_count = total - len(scored)
    bin_rows: list[dict[str, Any]] = []
    ece = 0.0
    mce = 0.0
    for index in range(bins):
        lower = index / bins
        upper = (index + 1) / bins
        members = [item for item in scored if index == bins - 1 and item[0] == 1.0 or lower <= item[0] < upper]
        predicted_mean = None if not members else _rounded(sum(item[0] for item in members) / len(members))
        observed_rate = None if not members else _rounded(sum(item[1] for item in members) / len(members))
        gap = None if not members else _rounded(abs(float(predicted_mean) - float(observed_rate)))
        fraction = 0 if not scored else _rounded(len(members) / len(scored))
        if gap is not None:
            ece += float(fraction) * float(gap)
            mce = max(mce, float(gap))
        bin_rows.append({
            "lower": _rounded(lower),
            "upper": _rounded(upper),
            "count": len(members),
            "predicted_mean": predicted_mean,
            "observed_rate": observed_rate,
            "absolute_gap": gap,
            "population_fraction": fraction,
        })
    predicted_positive = sum(score >= threshold for score, _ in scored)
    observed_positive = sum(label == 1 for _, label in scored)
    return {
        "total_count": total,
        "scored_count": len(scored),
        "unscored_count": unscored_count,
        "coverage": 0 if total == 0 else _rounded(len(scored) / total),
        "abstention_rate": 0 if total == 0 else _rounded(unscored_count / total),
        "brier_score": None if not scored else _rounded(sum((score - label) ** 2 for score, label in scored) / len(scored)),
        "expected_calibration_error": None if not scored else _rounded(ece),
        "maximum_calibration_error": None if not scored else _rounded(mce),
        "threshold_accuracy": None if not scored else _rounded(sum((score >= threshold) == (label == 1) for score, label in scored) / len(scored)),
        "predicted_positive_rate": None if not scored else _rounded(predicted_positive / len(scored)),
        "observed_positive_rate": None if not scored else _rounded(observed_positive / len(scored)),
        "bins": bin_rows,
    }


def _validate_metrics(value: Any, name: str, *, bins: int) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        _fail(f"{name} must be an object")
    expected = {
        "total_count", "scored_count", "unscored_count", "coverage", "abstention_rate",
        "brier_score", "expected_calibration_error", "maximum_calibration_error",
        "threshold_accuracy", "predicted_positive_rate", "observed_positive_rate", "bins",
    }
    if set(value).difference(expected):
        _fail(f"{name} contains unsupported fields")
    total = _bounded_integer(f"{name}.total_count", value.get("total_count"), 0, MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_CASES)
    scored = _bounded_integer(f"{name}.scored_count", value.get("scored_count"), 0, total)
    unscored = _bounded_integer(f"{name}.unscored_count", value.get("unscored_count"), 0, total)
    if scored + unscored != total:
        _fail(f"{name} scored and unscored counts are inconsistent")
    for field in ("coverage", "abstention_rate"):
        _bounded_float(f"{name}.{field}", value.get(field), 0, 1)
    for field in ("brier_score", "expected_calibration_error", "maximum_calibration_error", "threshold_accuracy", "predicted_positive_rate", "observed_positive_rate"):
        raw = value.get(field)
        if raw is not None:
            _bounded_float(f"{name}.{field}", raw, 0, 1)
    raw_bins = value.get("bins")
    if isinstance(raw_bins, (str, bytes)) or not isinstance(raw_bins, Sequence) or len(raw_bins) != bins:
        _fail(f"{name}.bins must contain exactly {bins} bins")
    normalized_bins: list[dict[str, Any]] = []
    count_sum = 0
    fraction_sum = 0.0
    for index, raw_bin in enumerate(raw_bins):
        if not isinstance(raw_bin, Mapping):
            _fail(f"{name}.bins[{index}] must be an object")
        if set(raw_bin).difference({"lower", "upper", "count", "predicted_mean", "observed_rate", "absolute_gap", "population_fraction"}):
            _fail(f"{name}.bins[{index}] contains unsupported fields")
        lower = _bounded_float(f"{name}.bins[{index}].lower", raw_bin.get("lower"), 0, 1)
        upper = _bounded_float(f"{name}.bins[{index}].upper", raw_bin.get("upper"), 0, 1)
        if lower != _rounded(index / bins) or upper != _rounded((index + 1) / bins) or lower >= upper:
            _fail(f"{name}.bins[{index}] bounds are invalid")
        count = _bounded_integer(f"{name}.bins[{index}].count", raw_bin.get("count"), 0, scored)
        for field in ("predicted_mean", "observed_rate", "absolute_gap"):
            raw = raw_bin.get(field)
            if raw is not None:
                _bounded_float(f"{name}.bins[{index}].{field}", raw, 0, 1)
        fraction = _bounded_float(f"{name}.bins[{index}].population_fraction", raw_bin.get("population_fraction"), 0, 1)
        if fraction != (0 if scored == 0 else _rounded(count / scored)):
            _fail(f"{name}.bins[{index}] population_fraction is inconsistent")
        if count == 0:
            if any(raw_bin.get(field) is not None for field in ("predicted_mean", "observed_rate", "absolute_gap")):
                _fail(f"{name}.bins[{index}] contains metrics for an empty bin")
        else:
            if any(raw_bin.get(field) is None for field in ("predicted_mean", "observed_rate", "absolute_gap")):
                _fail(f"{name}.bins[{index}] is missing populated-bin metrics")
            expected_gap = _rounded(abs(float(raw_bin["predicted_mean"]) - float(raw_bin["observed_rate"])))
            if raw_bin["absolute_gap"] != expected_gap:
                _fail(f"{name}.bins[{index}] absolute_gap is inconsistent")
        count_sum += count
        fraction_sum += fraction
        normalized_bins.append(dict(raw_bin))
    if count_sum != scored or (scored and abs(fraction_sum - 1) > 1e-9) or (not scored and fraction_sum != 0):
        _fail(f"{name}.bins do not partition scored observations")
    if value.get("coverage") != (0 if total == 0 else _rounded(scored / total)):
        _fail(f"{name}.coverage is inconsistent")
    if value.get("abstention_rate") != (0 if total == 0 else _rounded(unscored / total)):
        _fail(f"{name}.abstention_rate is inconsistent")
    if scored == 0:
        for field in ("brier_score", "expected_calibration_error", "maximum_calibration_error", "threshold_accuracy", "predicted_positive_rate", "observed_positive_rate"):
            if value.get(field) is not None:
                _fail(f"{name}.{field} must be null without scored observations")
    else:
        expected_ece = _rounded(sum(float(row["population_fraction"]) * float(row["absolute_gap"]) for row in normalized_bins if row["absolute_gap"] is not None))
        expected_mce = _rounded(max(float(row["absolute_gap"]) for row in normalized_bins if row["absolute_gap"] is not None))
        if value.get("expected_calibration_error") != expected_ece or value.get("maximum_calibration_error") != expected_mce:
            _fail(f"{name} calibration error metrics are inconsistent with bins")
    return deepcopy(dict(value))


def _domain_report_status(calibration: Mapping[str, Any], holdout: Mapping[str, Any], *, min_calibration: int, min_holdout: int, max_ece: float, max_brier: float) -> str:
    if calibration["unscored_count"] > 0:
        return "insufficient_calibration"
    if holdout["unscored_count"] > 0:
        return "insufficient_holdout"
    if calibration["scored_count"] < min_calibration:
        return "insufficient_calibration"
    if holdout["scored_count"] < min_holdout:
        return "insufficient_holdout"
    if (
        holdout["expected_calibration_error"] is None
        or holdout["brier_score"] is None
        or holdout["expected_calibration_error"] > max_ece
        or holdout["brier_score"] > max_brier
    ):
        return "miscalibrated"
    return "ready"


def _catalogue_digest(registry: DomainEvaluatorRegistry, domains: Sequence[str]) -> str:
    entries = []
    for domain in domains:
        adapter = registry.resolve_for_autonomous_domain(domain)
        entries.append({"domain": domain, "evaluator_id": adapter.evaluator_id, "evaluator_version": adapter.evaluator_version, "profile": adapter.profile.to_dict()})
    return content_digest(entries)


def _case_set_digest(cases: Sequence[Mapping[str, Any]]) -> str:
    return content_digest(sorted(({
        "case_id": case["case_id"],
        "domain": case["domain"],
        "case_digest": case["case_digest"],
        "split": case["split"],
        "label": case["label"],
    } for case in cases), key=lambda item: (item["domain"], item["case_id"])))


def _evaluation_digest(rows: Sequence[Mapping[str, Any]]) -> str:
    return content_digest(sorted((dict(row) for row in rows), key=lambda item: (item["domain"], item["case_id"])))


def validate_autonomous_evaluator_calibration_report(value: Mapping[str, Any]) -> dict[str, Any]:
    """Strictly validate a metadata-only calibration report and its digest."""

    if not isinstance(value, Mapping):
        _fail("report must be an object")
    try:
        encoded = canonical_json(value).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise ArgumentError("autonomous evaluator calibration report is not canonical JSON") from error
    if len(encoded) > MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_REPORT_BYTES:
        _fail(f"report exceeds {MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_REPORT_BYTES} bytes")
    required = {
        "schema", "status", "target_domains", "evaluator_catalogue_digest", "case_set_digest", "seed",
        "bins", "holdout_fraction", "min_calibration_cases_per_domain", "min_holdout_cases_per_domain",
        "max_expected_calibration_error", "max_brier_score", "require_all_domains", "missing_domains",
        "domains", "aggregate_calibration", "aggregate_holdout", "gate", "execution", "retention",
        "secret_material", "report_digest",
    }
    if set(value).difference(required) or value.get("schema") != AUTONOMOUS_EVALUATOR_CALIBRATION_SCHEMA:
        _fail("report schema or fields are invalid")
    if value.get("status") not in _REPORT_STATUSES:
        _fail("report status is invalid")
    targets = _domains(value.get("target_domains"))
    _digest("report evaluator_catalogue_digest", value.get("evaluator_catalogue_digest"))
    _digest("report case_set_digest", value.get("case_set_digest"))
    _bounded_text("report seed", value.get("seed"), MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_SEED_BYTES)
    bins = _bounded_integer("report bins", value.get("bins"), 1, MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_BINS)
    holdout_fraction = _bounded_float("report holdout_fraction", value.get("holdout_fraction"), 0, 0.9)
    min_calibration = _bounded_integer("report min_calibration_cases_per_domain", value.get("min_calibration_cases_per_domain"), 1, MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_CASES)
    min_holdout = _bounded_integer("report min_holdout_cases_per_domain", value.get("min_holdout_cases_per_domain"), 1, MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_CASES)
    max_ece = _bounded_float("report max_expected_calibration_error", value.get("max_expected_calibration_error"), 0, 1)
    max_brier = _bounded_float("report max_brier_score", value.get("max_brier_score"), 0, 1)
    if not isinstance(value.get("require_all_domains"), bool):
        _fail("report require_all_domains must be boolean")
    raw_domains = value.get("domains")
    if isinstance(raw_domains, (str, bytes)) or not isinstance(raw_domains, Sequence) or len(raw_domains) != len(targets):
        _fail("report domains are malformed")
    missing = _sequence("report missing_domains", value.get("missing_domains"), len(targets))
    if tuple(missing) != tuple(
        domain
        for domain in targets
        if not any(
            row.get("domain") == domain and row.get("case_count") > 0
            for row in raw_domains
            if isinstance(row, Mapping)
        )
    ):
        _fail("report missing_domains does not match domain rows")
    domain_rows: list[dict[str, Any]] = []
    for index, raw in enumerate(raw_domains):
        if not isinstance(raw, Mapping):
            _fail(f"report domain {index} must be an object")
        expected_fields = {"domain", "evaluator_id", "evaluator_version", "pass_threshold", "case_count", "calibration", "holdout", "status", "case_set_digest", "evaluation_digest", "error_count"}
        if set(raw).difference(expected_fields):
            _fail(f"report domain {index} contains unsupported fields")
        domain = raw.get("domain")
        if domain != targets[index]:
            _fail("report domains are not in canonical target order")
        _bounded_text(f"report domain {domain}.evaluator_id", raw.get("evaluator_id"), 256)
        _bounded_text(f"report domain {domain}.evaluator_version", raw.get("evaluator_version"), 128)
        _bounded_float(f"report domain {domain}.pass_threshold", raw.get("pass_threshold"), 0, 1)
        case_count = _bounded_integer(f"report domain {domain}.case_count", raw.get("case_count"), 0, MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_CASES)
        _validate_metrics(raw.get("calibration"), f"report domain {domain}.calibration", bins=bins)
        _validate_metrics(raw.get("holdout"), f"report domain {domain}.holdout", bins=bins)
        if raw.get("status") not in _DOMAIN_STATUSES:
            _fail(f"report domain {domain}.status is invalid")
        _digest(f"report domain {domain}.case_set_digest", raw.get("case_set_digest"))
        _digest(f"report domain {domain}.evaluation_digest", raw.get("evaluation_digest"))
        _bounded_integer(f"report domain {domain}.error_count", raw.get("error_count"), 0, case_count)
        if raw["calibration"]["total_count"] + raw["holdout"]["total_count"] != case_count:
            _fail(f"report domain {domain} case count is inconsistent")
        expected_status = _domain_report_status(raw["calibration"], raw["holdout"], min_calibration=min_calibration, min_holdout=min_holdout, max_ece=max_ece, max_brier=max_brier)
        if raw["status"] != expected_status:
            _fail(f"report domain {domain} status is inconsistent")
        domain_rows.append(deepcopy(dict(raw)))
    _validate_metrics(value.get("aggregate_calibration"), "report aggregate_calibration", bins=bins)
    _validate_metrics(value.get("aggregate_holdout"), "report aggregate_holdout", bins=bins)
    for split, field in (("calibration", "aggregate_calibration"), ("holdout", "aggregate_holdout")):
        rows = [row[split] for row in domain_rows]
        expected_total = sum(row["total_count"] for row in rows)
        expected_scored = sum(row["scored_count"] for row in rows)
        expected_unscored = sum(row["unscored_count"] for row in rows)
        aggregate = value[field]
        if aggregate["total_count"] != expected_total or aggregate["scored_count"] != expected_scored or aggregate["unscored_count"] != expected_unscored:
            _fail(f"report {field} aggregate counts are inconsistent")
        if aggregate["coverage"] != (0 if expected_total == 0 else _rounded(expected_scored / expected_total)) or aggregate["abstention_rate"] != (0 if expected_total == 0 else _rounded(expected_unscored / expected_total)):
            _fail(f"report {field} aggregate coverage is inconsistent")
        # Brier and observed-positive rates are threshold-independent.  Per-domain adapters
        # may use different pass thresholds, so threshold accuracy/predicted-positive rates are
        # intentionally validated only within each metric object.
        for metric in ("brier_score", "observed_positive_rate"):
            expected_metric = None if expected_scored == 0 else _rounded(sum(float(row[metric]) * row["scored_count"] for row in rows if row[metric] is not None) / expected_scored)
            if aggregate[metric] != expected_metric:
                _fail(f"report {field} aggregate {metric} is inconsistent")
    gate = value.get("gate")
    if not isinstance(gate, Mapping):
        _fail("report gate must be an object")
    gate_fields = {"required_domains", "missing_domains", "non_ready_domains", "decision", "reasons", "admit_learning", "hold_learning"}
    if set(gate).difference(gate_fields):
        _fail("report gate contains unsupported fields")
    if tuple(gate.get("required_domains", ())) != targets or tuple(gate.get("missing_domains", ())) != tuple(missing):
        _fail("report gate domain coverage is inconsistent")
    non_ready = tuple(row["domain"] for row in domain_rows if row["status"] != "ready")
    if tuple(gate.get("non_ready_domains", ())) != non_ready:
        _fail("report gate non-ready domains are inconsistent")
    if gate.get("decision") not in _ADMISSION_DECISIONS or not isinstance(gate.get("admit_learning"), bool) or not isinstance(gate.get("hold_learning"), bool) or gate["admit_learning"] == gate["hold_learning"]:
        _fail("report gate decision is invalid")
    if gate["decision"] != ("admit_learning" if gate["admit_learning"] else "hold_learning"):
        _fail("report gate decision flags are inconsistent")
    reasons = _sequence("report gate reasons", gate.get("reasons"), MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_REASON_COUNT)
    for index, reason in enumerate(reasons):
        _bounded_text(f"report gate reasons[{index}]", reason, 512)
    expected_status = "insufficient_coverage" if value["require_all_domains"] and missing else "insufficient_evidence" if any(row["status"] in {"insufficient_calibration", "insufficient_holdout"} for row in domain_rows) else "miscalibrated" if any(row["status"] == "miscalibrated" for row in domain_rows) else "ready"
    if value["status"] != expected_status:
        _fail("report status does not match domain gate state")
    if value.get("execution") != "provider_free;value_only_evaluator_replay;no_learning_mutation" or value.get("retention") != _RETENTION or value.get("secret_material") != _SECRET_MATERIAL:
        _fail("report execution or retention markers are invalid")
    report_digest = _digest("report report_digest", value.get("report_digest"))
    body = {key: item for key, item in value.items() if key != "report_digest"}
    if content_digest(body) != report_digest:
        _fail("report digest does not match its canonical projection")
    return deepcopy(dict(value))


def calibrate_autonomous_evaluators(
    cases: Sequence[Mapping[str, Any]],
    *,
    registry: DomainEvaluatorRegistry | None = None,
    domains: Sequence[str] | None = None,
    seed: str = "default",
    holdout_fraction: float = 0.2,
    bins: int = 10,
    min_calibration_cases_per_domain: int = 4,
    min_holdout_cases_per_domain: int = 2,
    max_expected_calibration_error: float = 0.15,
    max_brier_score: float = 0.15,
    require_all_domains: bool = True,
) -> dict[str, Any]:
    """Calibrate domain evaluators without retaining caller-owned cases."""

    if isinstance(cases, (str, bytes, bytearray)) or not isinstance(cases, Sequence):
        _fail("cases must be an array")
    if len(cases) > MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_CASES:
        _fail(f"cases contain more than {MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_CASES} entries")
    if registry is None:
        registry = DomainEvaluatorRegistry.with_builtin_autonomous_profiles()
    if not isinstance(registry, DomainEvaluatorRegistry):
        _fail("registry must be a DomainEvaluatorRegistry")
    target_domains = _domains(domains)
    seed_value = _bounded_text("seed", seed, MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_SEED_BYTES)
    fraction = _bounded_float("holdout_fraction", holdout_fraction, 0, 0.9)
    bin_count = _bounded_integer("bins", bins, 1, MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_BINS)
    min_calibration = _bounded_integer("min_calibration_cases_per_domain", min_calibration_cases_per_domain, 1, MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_CASES)
    min_holdout = _bounded_integer("min_holdout_cases_per_domain", min_holdout_cases_per_domain, 1, MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_CASES)
    max_ece = _bounded_float("max_expected_calibration_error", max_expected_calibration_error, 0, 1)
    max_brier = _bounded_float("max_brier_score", max_brier_score, 0, 1)
    if not isinstance(require_all_domains, bool):
        _fail("require_all_domains must be boolean")
    seen: set[str] = set()
    normalized_cases: list[dict[str, Any]] = []
    for index, value in enumerate(cases):
        normalized = _normalize_case(value, index, registry=registry, seed=seed_value, holdout_fraction=fraction)
        if normalized["case_id"] in seen:
            _fail(f"case id {normalized['case_id']} is duplicated")
        seen.add(normalized["case_id"])
        if normalized["domain"] in target_domains:
            normalized_cases.append(normalized)
    normalized_cases.sort(key=lambda item: (item["domain"], item["case_id"]))
    domain_reports: list[dict[str, Any]] = []
    evaluation_rows: list[dict[str, Any]] = []
    for domain in target_domains:
        adapter = registry.resolve_for_autonomous_domain(domain)
        domain_cases = [case for case in normalized_cases if case["domain"] == domain]
        observations = {"calibration": [], "holdout": []}
        errors = 0
        for case in domain_cases:
            score: float | None = None
            scored = False
            try:
                decision = adapter.assess_value_only_input({"evidence": case["evidence"], "context": case["context"], "evidence_digest": content_digest(case["evidence"])})
                score = _rounded(max(0.0, min(1.0, float(decision.reward))))
                scored = case["label"] in (0, 1) and decision.evidence_digest is not None
            except Exception:
                errors += 1
            observations[case["split"]].append((score if scored and score is not None else 0.0, case["label"] if scored else None))
            evaluation_rows.append({
                "case_id": case["case_id"],
                "domain": domain,
                "split": case["split"],
                "label": case["label"] if scored else None,
                "score": score if scored else None,
                "evaluator_id": adapter.evaluator_id,
                "evaluator_version": adapter.evaluator_version,
            })
        calibration_metrics = _metrics(observations["calibration"], bins=bin_count, threshold=adapter.profile.pass_threshold)
        holdout_metrics = _metrics(observations["holdout"], bins=bin_count, threshold=adapter.profile.pass_threshold)
        domain_evaluations = [row for row in evaluation_rows if row["domain"] == domain]
        domain_descriptor = [{"case_id": case["case_id"], "case_digest": case["case_digest"], "split": case["split"], "label": case["label"]} for case in domain_cases]
        domain_reports.append({
            "domain": domain,
            "evaluator_id": adapter.evaluator_id,
            "evaluator_version": adapter.evaluator_version,
            "pass_threshold": adapter.profile.pass_threshold,
            "case_count": len(domain_cases),
            "calibration": calibration_metrics,
            "holdout": holdout_metrics,
            "status": _domain_report_status(calibration_metrics, holdout_metrics, min_calibration=min_calibration, min_holdout=min_holdout, max_ece=max_ece, max_brier=max_brier),
            "case_set_digest": content_digest(domain_descriptor),
            "evaluation_digest": _evaluation_digest(domain_evaluations),
            "error_count": errors,
        })
    missing = [domain for domain in target_domains if not any(row["domain"] == domain and row["case_count"] > 0 for row in domain_reports)]
    non_ready = [row["domain"] for row in domain_reports if row["status"] != "ready"]
    reasons: list[str] = []
    reasons.extend(f"coverage:{domain}" for domain in missing)
    reasons.extend(f"domain:{row['domain']}:{row['status']}" for row in domain_reports if row["status"] != "ready")
    gate_decision = "admit_learning" if not missing and not non_ready and (not require_all_domains or not missing) else "hold_learning"
    gate = {
        "required_domains": list(target_domains),
        "missing_domains": missing,
        "non_ready_domains": non_ready,
        "decision": gate_decision,
        "reasons": reasons[:MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_REASON_COUNT],
        "admit_learning": gate_decision == "admit_learning",
        "hold_learning": gate_decision == "hold_learning",
    }
    # Aggregate from per-case evaluation rows so report validation does not need raw evidence.
    aggregate_calibration = _metrics(
        [(float(row["score"]), row["label"]) if row["score"] is not None else (0.0, None) for row in evaluation_rows if row["split"] == "calibration"],
        bins=bin_count,
        threshold=0.5,
    )
    aggregate_holdout = _metrics(
        [(float(row["score"]), row["label"]) if row["score"] is not None else (0.0, None) for row in evaluation_rows if row["split"] == "holdout"],
        bins=bin_count,
        threshold=0.5,
    )
    status = "insufficient_coverage" if require_all_domains and missing else "insufficient_evidence" if any(row["status"] in {"insufficient_calibration", "insufficient_holdout"} for row in domain_reports) else "miscalibrated" if any(row["status"] == "miscalibrated" for row in domain_reports) else "ready"
    body = {
        "schema": AUTONOMOUS_EVALUATOR_CALIBRATION_SCHEMA,
        "status": status,
        "target_domains": list(target_domains),
        "evaluator_catalogue_digest": _catalogue_digest(registry, target_domains),
        "case_set_digest": _case_set_digest(normalized_cases),
        "seed": seed_value,
        "bins": bin_count,
        "holdout_fraction": fraction,
        "min_calibration_cases_per_domain": min_calibration,
        "min_holdout_cases_per_domain": min_holdout,
        "max_expected_calibration_error": max_ece,
        "max_brier_score": max_brier,
        "require_all_domains": require_all_domains,
        "missing_domains": missing,
        "domains": domain_reports,
        "aggregate_calibration": aggregate_calibration,
        "aggregate_holdout": aggregate_holdout,
        "gate": gate,
        "execution": "provider_free;value_only_evaluator_replay;no_learning_mutation",
        "retention": _RETENTION,
        "secret_material": _SECRET_MATERIAL,
    }
    report = {**body, "report_digest": content_digest(body)}
    if len(canonical_json(report).encode("utf-8")) > MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_REPORT_BYTES:
        _fail(f"report exceeds {MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_REPORT_BYTES} bytes")
    return validate_autonomous_evaluator_calibration_report(report)


def replay_autonomous_evaluator_calibration(
    report: Mapping[str, Any],
    cases: Sequence[Mapping[str, Any]],
    *,
    registry: DomainEvaluatorRegistry | None = None,
) -> dict[str, Any]:
    """Replay a report configuration and expose explicit drift findings."""

    source = validate_autonomous_evaluator_calibration_report(report)
    replay = calibrate_autonomous_evaluators(
        cases,
        registry=registry,
        domains=source["target_domains"],
        seed=source["seed"],
        holdout_fraction=source["holdout_fraction"],
        bins=source["bins"],
        min_calibration_cases_per_domain=source["min_calibration_cases_per_domain"],
        min_holdout_cases_per_domain=source["min_holdout_cases_per_domain"],
        max_expected_calibration_error=source["max_expected_calibration_error"],
        max_brier_score=source["max_brier_score"],
        require_all_domains=source["require_all_domains"],
    )
    mismatches: list[str] = []
    for field in ("evaluator_catalogue_digest", "case_set_digest", "report_digest", "status"):
        expected = source[field]
        observed = replay[field]
        if expected != observed:
            mismatches.append(field)
    body = {
        "schema": AUTONOMOUS_EVALUATOR_CALIBRATION_REPLAY_SCHEMA,
        "source_report_digest": source["report_digest"],
        "replay_report_digest": replay["report_digest"],
        "evaluator_catalogue_match": source["evaluator_catalogue_digest"] == replay["evaluator_catalogue_digest"],
        "case_set_match": source["case_set_digest"] == replay["case_set_digest"],
        "matches": not mismatches,
        "mismatches": mismatches,
        "execution": "provider_free;value_only_replay;no_learning_mutation",
        "retention": "report_digests_and_drift_flags_only;cases_not_retained",
        "secret_material": _SECRET_MATERIAL,
    }
    return {**body, "replay_digest": content_digest(body)}


def admit_autonomous_evaluator_calibration(report: Mapping[str, Any], domain: str) -> dict[str, Any]:
    """Return a scoped learning admission decision for one autonomous domain."""

    normalized = validate_autonomous_evaluator_calibration_report(report)
    if not isinstance(domain, str) or domain not in normalized["target_domains"]:
        _fail("admission domain is not covered by the report")
    row = next(row for row in normalized["domains"] if row["domain"] == domain)
    reasons: list[str] = []
    if normalized["status"] != "ready":
        reasons.append(f"report:{normalized['status']}")
    if row["status"] != "ready":
        reasons.append(f"domain:{row['status']}")
    decision = "admit_learning" if not reasons and normalized["gate"]["decision"] == "admit_learning" else "hold_learning"
    body = {
        "schema": AUTONOMOUS_EVALUATOR_CALIBRATION_ADMISSION_SCHEMA,
        "domain": domain,
        "evaluator_id": row["evaluator_id"],
        "evaluator_version": row["evaluator_version"],
        "report_digest": normalized["report_digest"],
        "decision": decision,
        "reasons": reasons,
        "execution": "admission_projection_only;does_not_mutate_learning_or_invoke_provider",
        "retention": "domain_and_report_digests_only",
        "secret_material": _SECRET_MATERIAL,
    }
    return {**body, "admission_digest": content_digest(body)}


def assert_autonomous_evaluator_calibration_ready(report: Mapping[str, Any]) -> dict[str, Any]:
    normalized = validate_autonomous_evaluator_calibration_report(report)
    if normalized["gate"]["decision"] != "admit_learning":
        _fail("calibration gate is holding learning: " + ", ".join(normalized["gate"]["reasons"]))
    return normalized


class AutonomousEvaluatorCalibrationSnapshotTextStore(Protocol):
    def read(self) -> str | None: ...

    def write(self, value: str) -> None: ...


class TransactionalAutonomousEvaluatorCalibrationSnapshotTextStore(AutonomousEvaluatorCalibrationSnapshotTextStore, Protocol):
    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool: ...


class AutonomousEvaluatorCalibrationRegistry:
    """Bounded in-memory report registry keyed by a validated report digest."""

    def __init__(self, reports: Sequence[Mapping[str, Any]] = (), *, max_reports: int = MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_REGISTRY_REPORTS) -> None:
        self.max_reports = _bounded_integer("registry max_reports", max_reports, 1, MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_REGISTRY_REPORTS)
        self._reports: dict[str, dict[str, Any]] = {}
        self._lock = threading.RLock()
        for report in reports:
            self.register(report)

    def register(self, report: Mapping[str, Any]) -> str:
        normalized = validate_autonomous_evaluator_calibration_report(report)
        digest = normalized["report_digest"]
        with self._lock:
            existing = self._reports.get(digest)
            if existing is not None and existing != normalized:
                _fail("registry report digest collision detected")
            if existing is None and len(self._reports) >= self.max_reports:
                _fail("registry report count exceeds its bound")
            self._reports[digest] = normalized
        return digest

    def get(self, report_digest: str) -> dict[str, Any] | None:
        _digest("registry report_digest", report_digest)
        with self._lock:
            report = self._reports.get(report_digest)
            return None if report is None else deepcopy(report)

    def reports(self) -> list[dict[str, Any]]:
        with self._lock:
            return [deepcopy(self._reports[digest]) for digest in sorted(self._reports)]

    def snapshot(self) -> dict[str, Any]:
        with self._lock:
            body = {
                "schema": AUTONOMOUS_EVALUATOR_CALIBRATION_REGISTRY_SCHEMA,
                "reports": self.reports(),
                "retention": "validated_calibration_reports_only;source_cases_not_retained",
                "secret_material": _SECRET_MATERIAL,
            }
        snapshot = {**body, "snapshot_digest": content_digest(body)}
        if len(canonical_json(snapshot).encode("utf-8")) > MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_REGISTRY_BYTES:
            _fail("registry snapshot exceeds its byte bound")
        return snapshot

    def restore(self, snapshot: Mapping[str, Any]) -> None:
        normalized = validate_autonomous_evaluator_calibration_registry_snapshot(snapshot, max_reports=self.max_reports)
        with self._lock:
            self._reports = {}
            for report in normalized["reports"]:
                self.register(report)


def validate_autonomous_evaluator_calibration_registry_snapshot(value: Mapping[str, Any], *, max_reports: int = MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_REGISTRY_REPORTS) -> dict[str, Any]:
    if not isinstance(value, Mapping) or value.get("schema") != AUTONOMOUS_EVALUATOR_CALIBRATION_REGISTRY_SCHEMA:
        _fail("registry snapshot schema is invalid")
    _bounded_integer("registry max_reports", max_reports, 1, MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_REGISTRY_REPORTS)
    raw_reports = _sequence("registry reports", value.get("reports"), max_reports)
    if value.get("retention") != "validated_calibration_reports_only;source_cases_not_retained" or value.get("secret_material") != _SECRET_MATERIAL:
        _fail("registry snapshot retention markers are invalid")
    digests: set[str] = set()
    reports: list[dict[str, Any]] = []
    for raw in raw_reports:
        report = validate_autonomous_evaluator_calibration_report(raw)
        if report["report_digest"] in digests:
            _fail("registry snapshot contains duplicate report digests")
        digests.add(report["report_digest"])
        reports.append(report)
    expected = {key: item for key, item in value.items() if key != "snapshot_digest"}
    _digest("registry snapshot_digest", value.get("snapshot_digest"))
    if content_digest(expected) != value["snapshot_digest"]:
        _fail("registry snapshot digest is invalid")
    return deepcopy(dict(value))


class InMemoryAutonomousEvaluatorCalibrationPersistence:
    """Validated in-memory persistence with compare-and-swap fencing."""

    def __init__(self, initial: Mapping[str, Any] | None = None) -> None:
        self._snapshot: dict[str, Any] | None = None
        self._lock = threading.RLock()
        if initial is not None:
            self.write(initial)

    def read(self) -> dict[str, Any] | None:
        with self._lock:
            return None if self._snapshot is None else json.loads(canonical_json(self._snapshot))

    def write(self, snapshot: Mapping[str, Any]) -> None:
        normalized = validate_autonomous_evaluator_calibration_registry_snapshot(snapshot)
        with self._lock:
            self._snapshot = normalized

    def write_if_unchanged(self, expected_snapshot_digest: str | None, snapshot: Mapping[str, Any]) -> bool:
        _digest("expected snapshot digest", expected_snapshot_digest, allow_none=True)
        normalized = validate_autonomous_evaluator_calibration_registry_snapshot(snapshot)
        with self._lock:
            observed = None if self._snapshot is None else self._snapshot["snapshot_digest"]
            if observed != expected_snapshot_digest:
                return False
            self._snapshot = normalized
            return True


class JsonAutonomousEvaluatorCalibrationPersistence:
    """Canonical JSON persistence for calibration registries."""

    def __init__(self, store: AutonomousEvaluatorCalibrationSnapshotTextStore, *, max_reports: int = MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_REGISTRY_REPORTS, max_bytes: int = MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_REGISTRY_BYTES) -> None:
        if not callable(getattr(store, "read", None)) or not callable(getattr(store, "write", None)):
            _fail("JSON persistence requires a text store")
        self.store = store
        self.max_reports = _bounded_integer("JSON max_reports", max_reports, 1, MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_REGISTRY_REPORTS)
        self.max_bytes = _bounded_integer("JSON max_bytes", max_bytes, 1, MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_REGISTRY_BYTES)

    def _encode(self, snapshot: Mapping[str, Any]) -> str:
        normalized = validate_autonomous_evaluator_calibration_registry_snapshot(snapshot, max_reports=self.max_reports)
        encoded = canonical_json(normalized)
        if len(encoded.encode("utf-8")) > self.max_bytes:
            _fail("JSON snapshot exceeds its byte bound")
        return encoded

    def read(self) -> dict[str, Any] | None:
        encoded = self.store.read()
        if encoded is None:
            return None
        if not isinstance(encoded, str) or len(encoded.encode("utf-8")) > self.max_bytes:
            _fail("JSON snapshot exceeds its byte bound")
        try:
            value = json.loads(encoded)
        except (TypeError, ValueError) as error:
            raise ArgumentError("autonomous evaluator calibration JSON snapshot is invalid") from error
        if encoded != canonical_json(value):
            _fail("JSON snapshot is not canonical")
        validate_autonomous_evaluator_calibration_registry_snapshot(value, max_reports=self.max_reports)
        return value

    def write(self, snapshot: Mapping[str, Any]) -> None:
        self.store.write(self._encode(snapshot))


class TransactionalJsonAutonomousEvaluatorCalibrationPersistence(JsonAutonomousEvaluatorCalibrationPersistence):
    def __init__(self, store: TransactionalAutonomousEvaluatorCalibrationSnapshotTextStore, **kwargs: Any) -> None:
        super().__init__(store, **kwargs)
        if not callable(getattr(store, "write_if_unchanged", None)):
            _fail("transactional JSON persistence requires write_if_unchanged")
        self.store = store

    def write_if_unchanged(self, expected_snapshot_digest: str | None, snapshot: Mapping[str, Any]) -> bool:
        _digest("expected snapshot digest", expected_snapshot_digest, allow_none=True)
        return bool(self.store.write_if_unchanged(expected_snapshot_digest, self._encode(snapshot)))


class SQLiteAutonomousEvaluatorCalibrationPersistence:
    """Transactional SQLite registry persistence for multi-process local workers."""

    def __init__(self, path: str | Path, *, max_reports: int = MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_REGISTRY_REPORTS, busy_timeout_ms: int = 5_000) -> None:
        if not isinstance(path, (str, Path)) or not str(path):
            _fail("SQLite path must be non-empty")
        self.path = str(path)
        self.max_reports = _bounded_integer("SQLite max_reports", max_reports, 1, MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_REGISTRY_REPORTS)
        self.busy_timeout_ms = _bounded_integer("SQLite busy_timeout_ms", busy_timeout_ms, 1, 120_000)
        self._lock = threading.RLock()
        if self.path != ":memory:":
            Path(self.path).parent.mkdir(parents=True, exist_ok=True)
        try:
            self._connection = sqlite3.connect(self.path, isolation_level=None, check_same_thread=False)
            self._connection.row_factory = sqlite3.Row
            self._connection.execute("PRAGMA synchronous=FULL")
            self._connection.execute(f"PRAGMA busy_timeout={self.busy_timeout_ms}")
            self._connection.execute("CREATE TABLE IF NOT EXISTS autonomous_evaluator_calibration_snapshots (singleton INTEGER PRIMARY KEY CHECK(singleton = 1), persistence_schema TEXT NOT NULL, schema TEXT NOT NULL, snapshot_json TEXT NOT NULL, snapshot_digest TEXT NOT NULL)")
        except sqlite3.Error as error:
            raise ArgumentError("could not initialize autonomous evaluator calibration SQLite persistence") from error

    def close(self) -> None:
        with self._lock:
            self._connection.close()

    def __enter__(self) -> "SQLiteAutonomousEvaluatorCalibrationPersistence":
        return self

    def __exit__(self, *_: Any) -> None:
        self.close()

    def read(self) -> dict[str, Any] | None:
        with self._lock:
            try:
                row = self._connection.execute("SELECT persistence_schema, schema, snapshot_json, snapshot_digest FROM autonomous_evaluator_calibration_snapshots WHERE singleton = 1").fetchone()
            except sqlite3.Error as error:
                raise ArgumentError("could not read autonomous evaluator calibration SQLite persistence") from error
        if row is None:
            return None
        if row["persistence_schema"] != AUTONOMOUS_EVALUATOR_CALIBRATION_SQLITE_SCHEMA or row["schema"] != AUTONOMOUS_EVALUATOR_CALIBRATION_REGISTRY_SCHEMA:
            _fail("SQLite snapshot schema is invalid")
        try:
            value = json.loads(row["snapshot_json"])
        except (TypeError, ValueError) as error:
            raise ArgumentError("autonomous evaluator calibration SQLite snapshot is invalid") from error
        if value.get("snapshot_digest") != row["snapshot_digest"]:
            _fail("SQLite snapshot digest is invalid")
        return validate_autonomous_evaluator_calibration_registry_snapshot(value, max_reports=self.max_reports)

    def _normalized(self, snapshot: Mapping[str, Any]) -> tuple[dict[str, Any], str]:
        value = validate_autonomous_evaluator_calibration_registry_snapshot(snapshot, max_reports=self.max_reports)
        return value, canonical_json(value)

    def write(self, snapshot: Mapping[str, Any]) -> None:
        value, encoded = self._normalized(snapshot)
        with self._lock:
            try:
                self._connection.execute("BEGIN IMMEDIATE")
                self._connection.execute("INSERT INTO autonomous_evaluator_calibration_snapshots(singleton,persistence_schema,schema,snapshot_json,snapshot_digest) VALUES(1,?,?,?,?) ON CONFLICT(singleton) DO UPDATE SET persistence_schema=excluded.persistence_schema,schema=excluded.schema,snapshot_json=excluded.snapshot_json,snapshot_digest=excluded.snapshot_digest", (AUTONOMOUS_EVALUATOR_CALIBRATION_SQLITE_SCHEMA, AUTONOMOUS_EVALUATOR_CALIBRATION_REGISTRY_SCHEMA, encoded, value["snapshot_digest"]))
                self._connection.execute("COMMIT")
            except sqlite3.Error as error:
                try:
                    self._connection.execute("ROLLBACK")
                except sqlite3.Error:
                    pass
                raise ArgumentError("could not write autonomous evaluator calibration SQLite persistence") from error

    def write_if_unchanged(self, expected_snapshot_digest: str | None, snapshot: Mapping[str, Any]) -> bool:
        _digest("expected snapshot digest", expected_snapshot_digest, allow_none=True)
        value, encoded = self._normalized(snapshot)
        with self._lock:
            try:
                self._connection.execute("BEGIN IMMEDIATE")
                row = self._connection.execute("SELECT snapshot_digest FROM autonomous_evaluator_calibration_snapshots WHERE singleton=1").fetchone()
                observed = None if row is None else row["snapshot_digest"]
                if observed != expected_snapshot_digest:
                    self._connection.execute("ROLLBACK")
                    return False
                self._connection.execute("INSERT INTO autonomous_evaluator_calibration_snapshots(singleton,persistence_schema,schema,snapshot_json,snapshot_digest) VALUES(1,?,?,?,?) ON CONFLICT(singleton) DO UPDATE SET persistence_schema=excluded.persistence_schema,schema=excluded.schema,snapshot_json=excluded.snapshot_json,snapshot_digest=excluded.snapshot_digest", (AUTONOMOUS_EVALUATOR_CALIBRATION_SQLITE_SCHEMA, AUTONOMOUS_EVALUATOR_CALIBRATION_REGISTRY_SCHEMA, encoded, value["snapshot_digest"]))
                self._connection.execute("COMMIT")
                return True
            except sqlite3.Error as error:
                try:
                    self._connection.execute("ROLLBACK")
                except sqlite3.Error:
                    pass
                raise ArgumentError("could not compare-and-swap autonomous evaluator calibration SQLite persistence") from error


class AutonomousEvaluatorCalibrationRegistryPersistenceCoordinator:
    """Restore/flush a registry with optional stale-writer fencing."""

    def __init__(self, registry: AutonomousEvaluatorCalibrationRegistry, persistence: Any) -> None:
        if not isinstance(registry, AutonomousEvaluatorCalibrationRegistry) or not callable(getattr(persistence, "read", None)) or not callable(getattr(persistence, "write", None)):
            _fail("registry persistence coordinator arguments are malformed")
        self.registry = registry
        self.persistence = persistence
        self._expected_snapshot_digest: str | None = None
        self._lock = threading.RLock()

    def restore(self) -> dict[str, Any]:
        with self._lock:
            snapshot = self.persistence.read()
            if snapshot is None:
                self._expected_snapshot_digest = None
                return {"status": "empty", "snapshot_digest": None, "reports": 0}
            self.registry.restore(snapshot)
            self._expected_snapshot_digest = snapshot["snapshot_digest"]
            return {"status": "restored", "snapshot_digest": self._expected_snapshot_digest, "reports": len(snapshot["reports"])}

    def flush(self) -> dict[str, Any]:
        with self._lock:
            snapshot = self.registry.snapshot()
            cas = getattr(self.persistence, "write_if_unchanged", None)
            if callable(cas) and not cas(self._expected_snapshot_digest, snapshot):
                _fail("registry persistence compare-and-swap conflict")
            if not callable(cas):
                self.persistence.write(snapshot)
            self._expected_snapshot_digest = snapshot["snapshot_digest"]
            return snapshot


__all__ = [
    "AUTONOMOUS_EVALUATOR_CALIBRATION_SCHEMA",
    "AUTONOMOUS_EVALUATOR_CALIBRATION_REPLAY_SCHEMA",
    "AUTONOMOUS_EVALUATOR_CALIBRATION_ADMISSION_SCHEMA",
    "AUTONOMOUS_EVALUATOR_CALIBRATION_REGISTRY_SCHEMA",
    "AUTONOMOUS_EVALUATOR_CALIBRATION_SQLITE_SCHEMA",
    "MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_CASES",
    "MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_BINS",
    "MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_DOMAINS",
    "MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_REASON_COUNT",
    "MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_REPORT_BYTES",
    "MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_REGISTRY_REPORTS",
    "MAX_AUTONOMOUS_EVALUATOR_CALIBRATION_REGISTRY_BYTES",
    "calibrate_autonomous_evaluators",
    "replay_autonomous_evaluator_calibration",
    "admit_autonomous_evaluator_calibration",
    "assert_autonomous_evaluator_calibration_ready",
    "validate_autonomous_evaluator_calibration_report",
    "validate_autonomous_evaluator_calibration_registry_snapshot",
    "AutonomousEvaluatorCalibrationSnapshotTextStore",
    "TransactionalAutonomousEvaluatorCalibrationSnapshotTextStore",
    "AutonomousEvaluatorCalibrationRegistry",
    "InMemoryAutonomousEvaluatorCalibrationPersistence",
    "JsonAutonomousEvaluatorCalibrationPersistence",
    "TransactionalJsonAutonomousEvaluatorCalibrationPersistence",
    "SQLiteAutonomousEvaluatorCalibrationPersistence",
    "AutonomousEvaluatorCalibrationRegistryPersistenceCoordinator",
]
