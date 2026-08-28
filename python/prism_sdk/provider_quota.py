"""Bounded provider/model quota admission for the real LLM transport.

The quota controller is deliberately smaller than a billing system. It accounts for provider
attempts and caller-supplied token/cost estimates, reserves concurrency before dispatch, replaces
estimates with provider usage at settlement, and persists only counters and policy metadata. It
never receives prompts, response text, headers, credentials, tool arguments, or effect values.
"""

from __future__ import annotations

from dataclasses import dataclass, field
import json
import math
import threading
import time
from typing import Any, Mapping, Protocol

from .authoring import content_digest
from .llm_runtime import ProviderError


PROVIDER_QUOTA_SCHEMA = "bioprism-provider-quota/0.1"
PROVIDER_QUOTA_SNAPSHOT_SCHEMA = "bioprism-provider-quota-snapshot/0.1"
PROVIDER_QUOTA_RETENTION = (
    "metadata_only;provider_model_counters_no_prompts_credentials_or_payloads"
)
PROVIDER_QUOTA_SECRET_MATERIAL = "never_returned"
MAX_PROVIDER_QUOTA_POLICIES = 256
MAX_PROVIDER_QUOTA_BUCKETS = 2048
MAX_PROVIDER_QUOTA_SNAPSHOT_BYTES = 2_000_000
MAX_PROVIDER_QUOTA_WINDOW_SECONDS = 7 * 24 * 60 * 60
MAX_PROVIDER_QUOTA_METRIC = 2_000_000_000
MAX_PROVIDER_QUOTA_COST_UNITS = 1_000_000_000
MAX_PROVIDER_QUOTA_TIMESTAMP = 2**53 - 1


def _epoch_ms() -> float:
    return time.time() * 1000.0


def _text(name: str, value: Any, maximum: int) -> str:
    if (
        not isinstance(value, str)
        or not value.strip()
        or len(value.encode("utf-8")) > maximum
        or any(ord(char) < 32 for char in value)
    ):
        raise ValueError(f"{name} must be a bounded non-empty identifier")
    return value.strip()


def _optional_text(name: str, value: Any, maximum: int) -> str | None:
    if value is None:
        return None
    return _text(name, value, maximum)


def _integer(name: str, value: Any, maximum: int, minimum: int = 0) -> int:
    if (
        isinstance(value, bool)
        or not isinstance(value, int)
        or value < minimum
        or value > maximum
    ):
        raise ValueError(f"{name} must be an integer within [{minimum}, {maximum}]")
    return value


def _optional_integer(name: str, value: Any, maximum: int) -> int | None:
    if value is None:
        return None
    return _integer(name, value, maximum)


def _number(name: str, value: Any, maximum: float) -> float | int:
    if (
        isinstance(value, bool)
        or not isinstance(value, (int, float))
        or not math.isfinite(float(value))
        or float(value) < 0
        or float(value) > maximum
    ):
        raise ValueError(f"{name} must be finite within [0, {maximum}]")
    numeric = float(value)
    return int(numeric) if numeric.is_integer() else numeric


def _optional_number(name: str, value: Any, maximum: float) -> float | None:
    if value is None:
        return None
    return _number(name, value, maximum)


def _policy_id(provider: str, model: str | None) -> str:
    return provider if model is None else f"{provider}/{model}"


def _policy(value: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise ValueError("provider quota policy must be an object")
    provider = _text("provider quota provider", value.get("provider"), 128)
    model = _optional_text("provider quota model", value.get("model"), 512)
    window_ms = value.get("window_ms", value.get("windowMs"))
    window_ms = _integer(
        "provider quota windowMs",
        window_ms,
        MAX_PROVIDER_QUOTA_WINDOW_SECONDS * 1000,
        1,
    )
    max_requests = _optional_integer(
        "provider quota maxRequests",
        value.get("max_requests", value.get("maxRequests")),
        MAX_PROVIDER_QUOTA_METRIC,
    )
    max_input_tokens = _optional_integer(
        "provider quota maxInputTokens",
        value.get("max_input_tokens", value.get("maxInputTokens")),
        MAX_PROVIDER_QUOTA_METRIC,
    )
    max_output_tokens = _optional_integer(
        "provider quota maxOutputTokens",
        value.get("max_output_tokens", value.get("maxOutputTokens")),
        MAX_PROVIDER_QUOTA_METRIC,
    )
    max_total_tokens = _optional_integer(
        "provider quota maxTotalTokens",
        value.get("max_total_tokens", value.get("maxTotalTokens")),
        MAX_PROVIDER_QUOTA_METRIC,
    )
    max_cost_units = _optional_number(
        "provider quota maxCostUnits",
        value.get("max_cost_units", value.get("maxCostUnits")),
        MAX_PROVIDER_QUOTA_COST_UNITS,
    )
    max_concurrent = _optional_integer(
        "provider quota maxConcurrent",
        value.get("max_concurrent", value.get("maxConcurrent")),
        MAX_PROVIDER_QUOTA_METRIC,
    )
    if all(
        item is None
        for item in (
            max_requests,
            max_input_tokens,
            max_output_tokens,
            max_total_tokens,
            max_cost_units,
            max_concurrent,
        )
    ):
        raise ValueError("provider quota policy must define at least one limit")
    return {
        "policy_id": _policy_id(provider, model),
        "provider": provider,
        "model": model,
        "window_ms": window_ms,
        "max_requests": max_requests,
        "max_input_tokens": max_input_tokens,
        "max_output_tokens": max_output_tokens,
        "max_total_tokens": max_total_tokens,
        "max_cost_units": max_cost_units,
        "max_concurrent": max_concurrent,
    }


def _metrics(
    requests: int = 0,
    input_tokens: int = 0,
    output_tokens: int = 0,
    cost_units: float | int = 0,
) -> dict[str, float | int]:
    return {
        "requests": requests,
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "cost_units": cost_units,
    }


def _add(
    left: Mapping[str, float | int], right: Mapping[str, float | int]
) -> dict[str, float | int]:
    return {
        key: left[key] + right[key]
        for key in ("requests", "input_tokens", "output_tokens", "cost_units")
    }


def _sub(
    left: Mapping[str, float | int], right: Mapping[str, float | int]
) -> dict[str, float | int]:
    return {
        key: max(0, left[key] - right[key])
        for key in ("requests", "input_tokens", "output_tokens", "cost_units")
    }


def _estimate(value: Mapping[str, Any]) -> dict[str, float | int]:
    provider = _text("provider quota request provider", value.get("provider"), 128)
    model = _text("provider quota request model", value.get("model"), 512)
    del provider, model
    input_tokens = _integer(
        "provider quota request inputTokens",
        value.get("input_tokens", value.get("inputTokens")),
        MAX_PROVIDER_QUOTA_METRIC,
    )
    output_tokens = _integer(
        "provider quota request outputTokens",
        value.get("output_tokens", value.get("outputTokens")),
        MAX_PROVIDER_QUOTA_METRIC,
    )
    cost_units = _number(
        "provider quota request costUnits",
        value.get("cost_units", value.get("costUnits", 0.0)),
        MAX_PROVIDER_QUOTA_COST_UNITS,
    )
    if input_tokens + output_tokens > MAX_PROVIDER_QUOTA_METRIC:
        raise ValueError("provider quota request total tokens exceed the bound")
    return _metrics(1, input_tokens, output_tokens, cost_units)


def _actual(
    estimate: Mapping[str, float | int], value: Mapping[str, Any] | None
) -> dict[str, float | int]:
    supplied = value or {}
    input_tokens = (
        estimate["input_tokens"]
        if supplied.get("input_tokens", supplied.get("inputTokens")) is None
        else _integer(
            "provider quota settlement inputTokens",
            supplied.get("input_tokens", supplied.get("inputTokens")),
            MAX_PROVIDER_QUOTA_METRIC,
        )
    )
    output_tokens = (
        estimate["output_tokens"]
        if supplied.get("output_tokens", supplied.get("outputTokens")) is None
        else _integer(
            "provider quota settlement outputTokens",
            supplied.get("output_tokens", supplied.get("outputTokens")),
            MAX_PROVIDER_QUOTA_METRIC,
        )
    )
    cost_value = supplied.get("cost_units", supplied.get("costUnits"))
    cost_units = (
        estimate["cost_units"]
        if cost_value is None
        else _number(
            "provider quota settlement costUnits",
            cost_value,
            MAX_PROVIDER_QUOTA_COST_UNITS,
        )
    )
    if input_tokens + output_tokens > MAX_PROVIDER_QUOTA_METRIC:
        raise ValueError("provider quota settlement total tokens exceed the bound")
    return _metrics(1, input_tokens, output_tokens, cost_units)


def _over_limit(
    policy: Mapping[str, Any], metrics: Mapping[str, float | int], concurrent: int
) -> list[str]:
    reasons: list[str] = []
    if (
        policy["max_requests"] is not None
        and metrics["requests"] > policy["max_requests"]
    ):
        reasons.append("requests")
    if (
        policy["max_input_tokens"] is not None
        and metrics["input_tokens"] > policy["max_input_tokens"]
    ):
        reasons.append("input_tokens")
    if (
        policy["max_output_tokens"] is not None
        and metrics["output_tokens"] > policy["max_output_tokens"]
    ):
        reasons.append("output_tokens")
    if (
        policy["max_total_tokens"] is not None
        and metrics["input_tokens"] + metrics["output_tokens"]
        > policy["max_total_tokens"]
    ):
        reasons.append("total_tokens")
    if (
        policy["max_cost_units"] is not None
        and metrics["cost_units"] > policy["max_cost_units"]
    ):
        reasons.append("cost_units")
    if policy["max_concurrent"] is not None and concurrent > policy["max_concurrent"]:
        reasons.append("concurrent")
    return reasons


@dataclass(slots=True)
class _Bucket:
    policy_id: str
    window_start: int
    used: dict[str, float | int] = field(default_factory=_metrics)
    reserved: dict[str, float | int] = field(default_factory=_metrics)


class ProviderQuotaError(ProviderError):
    """Retryable, metadata-only refusal emitted before provider dispatch."""

    def __init__(
        self,
        provider: str,
        model: str,
        policy: Mapping[str, Any],
        dimensions: list[str],
        retry_after_ms: int | None,
        observed: Mapping[str, Any],
        concurrent: int,
    ) -> None:
        super().__init__(
            f"provider quota exceeded for {provider}/{model}: {','.join(dimensions)}",
            retryable=True,
            status_code=429,
        )
        self.code = "quota_exceeded"
        self.provider = provider
        self.model = model
        self.operation = "quota_admission"
        self.retry_after_ms = retry_after_ms
        self.policy_id = str(policy["policy_id"])
        self.dimensions = tuple(sorted(dimensions))
        self.observed = dict(observed)
        self.concurrent = concurrent


class ProviderQuotaReservation:
    """One in-memory reservation spanning approval, transport, and settlement."""

    def __init__(
        self,
        controller: "ProviderQuotaController",
        provider: str,
        model: str,
        estimate: Mapping[str, float | int],
        entries: list[tuple[Mapping[str, Any], _Bucket]],
        reservation_id: str,
    ) -> None:
        self._controller = controller
        self.provider = provider
        self.model = model
        self.reservation_id = reservation_id
        self._estimate = dict(estimate)
        self._entries = entries
        self._active = True
        self._dispatched = False
        self._settlement: dict[str, Any] | None = None

    @property
    def is_dispatched(self) -> bool:
        return self._dispatched

    @property
    def estimate(self) -> dict[str, float | int]:
        return dict(self._estimate)

    def mark_dispatched(self) -> None:
        if not self._active:
            raise ProviderError("provider quota reservation is no longer active")
        self._dispatched = True

    def release(self) -> None:
        if not self._active:
            return
        self._active = False
        self._controller._release(self)

    def settle(self, actual: Mapping[str, Any] | None = None) -> dict[str, Any]:
        if self._settlement is not None:
            return dict(self._settlement)
        if not self._active:
            raise ProviderError("provider quota reservation was released")
        if not self._dispatched:
            raise ProviderError(
                "provider quota reservation must be marked dispatched before settlement"
            )
        self._active = False
        self._settlement = self._controller._settle(
            self, _actual(self._estimate, actual)
        )
        return dict(self._settlement)


class ProviderQuotaSnapshotTextStore(Protocol):
    def read(self) -> str | None: ...
    def write(self, value: str) -> None: ...


class TransactionalProviderQuotaSnapshotTextStore(
    ProviderQuotaSnapshotTextStore, Protocol
):
    def write_if_unchanged(
        self, expected_snapshot_digest: str | None, value: str
    ) -> bool: ...


class ProviderQuotaPersistence(Protocol):
    def read(self) -> Mapping[str, Any] | None: ...
    def write(self, snapshot: Mapping[str, Any]) -> None: ...


def _canonical_snapshot_bucket(value: Mapping[str, Any]) -> dict[str, Any]:
    allowed = {
        "policy_id",
        "window_start",
        "requests",
        "input_tokens",
        "output_tokens",
        "cost_units",
    }
    if set(value) != allowed:
        raise ProviderError(
            "provider quota snapshot bucket contains unsupported fields"
        )
    return {
        "policy_id": _text(
            "provider quota bucket policy_id", value.get("policy_id"), 640
        ),
        "window_start": _integer(
            "provider quota bucket window_start",
            value.get("window_start"),
            MAX_PROVIDER_QUOTA_TIMESTAMP,
        ),
        "requests": _integer(
            "provider quota bucket requests",
            value.get("requests"),
            MAX_PROVIDER_QUOTA_METRIC,
        ),
        "input_tokens": _integer(
            "provider quota bucket input_tokens",
            value.get("input_tokens"),
            MAX_PROVIDER_QUOTA_METRIC,
        ),
        "output_tokens": _integer(
            "provider quota bucket output_tokens",
            value.get("output_tokens"),
            MAX_PROVIDER_QUOTA_METRIC,
        ),
        "cost_units": _number(
            "provider quota bucket cost_units",
            value.get("cost_units"),
            MAX_PROVIDER_QUOTA_COST_UNITS,
        ),
    }


def validate_provider_quota_snapshot(value: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        raise ProviderError("provider quota snapshot must be an object")
    allowed = {
        "schema",
        "snapshot_generation",
        "previous_snapshot_digest",
        "policies",
        "buckets",
        "snapshot_digest",
        "retention",
        "secret_material",
    }
    if set(value) != allowed:
        raise ProviderError("provider quota snapshot contains unsupported fields")
    if (
        value.get("schema") != PROVIDER_QUOTA_SNAPSHOT_SCHEMA
        or value.get("retention") != PROVIDER_QUOTA_RETENTION
        or value.get("secret_material") != PROVIDER_QUOTA_SECRET_MATERIAL
    ):
        raise ProviderError("provider quota snapshot markers are invalid")
    generation = _integer(
        "provider quota snapshot_generation",
        value.get("snapshot_generation"),
        MAX_PROVIDER_QUOTA_METRIC,
        1,
    )
    previous = value.get("previous_snapshot_digest")
    if previous is not None and (
        not isinstance(previous, str)
        or len(previous) != 64
        or any(char not in "0123456789abcdef" for char in previous)
    ):
        raise ProviderError("provider quota previous snapshot digest is invalid")
    if (generation == 1) != (previous is None):
        raise ProviderError("provider quota snapshot chain is inconsistent")
    raw_policies = value.get("policies")
    raw_buckets = value.get("buckets")
    if (
        not isinstance(raw_policies, list)
        or len(raw_policies) > MAX_PROVIDER_QUOTA_POLICIES
        or not isinstance(raw_buckets, list)
        or len(raw_buckets) > MAX_PROVIDER_QUOTA_BUCKETS
    ):
        raise ProviderError("provider quota snapshot capacity is exceeded")
    policies = []
    policy_ids: set[str] = set()
    for raw in raw_policies:
        if not isinstance(raw, Mapping) or set(raw) != {
            "policy_id",
            "provider",
            "model",
            "window_ms",
            "max_requests",
            "max_input_tokens",
            "max_output_tokens",
            "max_total_tokens",
            "max_cost_units",
            "max_concurrent",
        }:
            raise ProviderError("provider quota snapshot policy is malformed")
        normalized = _policy(raw)
        if (
            raw.get("policy_id") != normalized["policy_id"]
            or normalized["policy_id"] in policy_ids
        ):
            raise ProviderError("provider quota snapshot policy identity is invalid")
        policy_ids.add(normalized["policy_id"])
        policies.append(normalized)
    buckets = []
    bucket_ids: set[str] = set()
    for raw in raw_buckets:
        if not isinstance(raw, Mapping):
            raise ProviderError("provider quota snapshot bucket is malformed")
        bucket = _canonical_snapshot_bucket(raw)
        if bucket["policy_id"] not in policy_ids or bucket["policy_id"] in bucket_ids:
            raise ProviderError("provider quota snapshot bucket identity is invalid")
        policy = next(
            item for item in policies if item["policy_id"] == bucket["policy_id"]
        )
        if bucket["window_start"] % policy["window_ms"] != 0:
            raise ProviderError(
                "provider quota snapshot bucket window is not canonical"
            )
        if _over_limit(
            policy,
            {
                "requests": bucket["requests"],
                "input_tokens": bucket["input_tokens"],
                "output_tokens": bucket["output_tokens"],
                "cost_units": bucket["cost_units"],
            },
            0,
        ):
            raise ProviderError("provider quota snapshot bucket exceeds its policy")
        bucket_ids.add(bucket["policy_id"])
        buckets.append(bucket)
    snapshot_digest = value.get("snapshot_digest")
    if (
        not isinstance(snapshot_digest, str)
        or len(snapshot_digest) != 64
        or any(char not in "0123456789abcdef" for char in snapshot_digest)
    ):
        raise ProviderError("provider quota snapshot digest is invalid")
    descriptor = {
        "schema": PROVIDER_QUOTA_SNAPSHOT_SCHEMA,
        "snapshot_generation": generation,
        "previous_snapshot_digest": previous,
        "policies": sorted(policies, key=lambda item: item["policy_id"]),
        "buckets": sorted(
            buckets, key=lambda item: (item["policy_id"], item["window_start"])
        ),
        "retention": PROVIDER_QUOTA_RETENTION,
        "secret_material": PROVIDER_QUOTA_SECRET_MATERIAL,
    }
    if content_digest(descriptor) != snapshot_digest:
        raise ProviderError("provider quota snapshot digest mismatch")
    result = {**descriptor, "snapshot_digest": snapshot_digest}
    if (
        len(
            json.dumps(
                result,
                ensure_ascii=False,
                separators=(",", ":"),
                sort_keys=True,
                allow_nan=False,
            ).encode("utf-8")
        )
        > MAX_PROVIDER_QUOTA_SNAPSHOT_BYTES
    ):
        raise ProviderError("provider quota snapshot exceeds its byte bound")
    return result


class ProviderQuotaController:
    """Thread-safe fixed-window quota admission shared by all autonomous domains."""

    def __init__(self, *, clock: Any | None = None) -> None:
        if clock is not None and not callable(clock):
            raise ValueError("provider quota clock must be callable")
        self._clock = _epoch_ms if clock is None else clock
        self._lock = threading.RLock()
        self._policies: dict[str, dict[str, Any]] = {}
        self._buckets: dict[str, _Bucket] = {}
        self._active: dict[str, int] = {}
        self._sequence = 0
        self._generation = 0
        self._previous_digest: str | None = None

    def set_policy(self, value: Mapping[str, Any]) -> dict[str, Any]:
        with self._lock:
            normalized = _policy(value)
            policy_id = normalized["policy_id"]
            if (
                len(self._policies) >= MAX_PROVIDER_QUOTA_POLICIES
                and policy_id not in self._policies
            ):
                raise ValueError("provider quota policy capacity is exhausted")
            if self._active.get(policy_id, 0) > 0:
                raise ValueError(
                    "cannot replace a provider quota policy with active reservations"
                )
            self._policies[policy_id] = normalized
            self._buckets.pop(policy_id, None)
            return dict(normalized)

    def remove_policy(self, provider: str, model: str | None = None) -> bool:
        provider = _text("provider quota provider", provider, 128)
        model = _optional_text("provider quota model", model, 512)
        policy_id = _policy_id(provider, model)
        with self._lock:
            if self._active.get(policy_id, 0) > 0:
                raise ValueError(
                    "cannot remove a provider quota policy with active reservations"
                )
            self._buckets.pop(policy_id, None)
            return self._policies.pop(policy_id, None) is not None

    def policies(self) -> list[dict[str, Any]]:
        with self._lock:
            return [dict(self._policies[key]) for key in sorted(self._policies)]

    def reserve(
        self, value: Mapping[str, Any], *, now: float | None = None
    ) -> ProviderQuotaReservation:
        provider = _text("provider quota request provider", value.get("provider"), 128)
        model = _text("provider quota request model", value.get("model"), 512)
        current_time = self._read_clock() if now is None else float(now)
        if not math.isfinite(current_time) or current_time < 0:
            raise ValueError("provider quota reservation time is invalid")
        estimate = _estimate(value)
        with self._lock:
            policies = [
                policy
                for policy in (
                    self._policies.get(_policy_id(provider, None)),
                    self._policies.get(_policy_id(provider, model)),
                )
                if policy is not None
            ]
            policies.sort(key=lambda item: item["policy_id"])
            entries: list[tuple[Mapping[str, Any], _Bucket]] = []
            for policy in policies:
                bucket = self._current_bucket(policy, current_time)
                projected = _add(bucket.used, _add(bucket.reserved, estimate))
                concurrent = self._active.get(policy["policy_id"], 0) + 1
                reasons = _over_limit(policy, projected, concurrent)
                if reasons:
                    retry_after_ms = (
                        None
                        if all(reason == "concurrent" for reason in reasons)
                        else max(
                            0,
                            int(
                                (
                                    bucket.window_start
                                    + policy["window_ms"]
                                    - current_time
                                )
                                * 1000
                            ),
                        )
                    )
                    raise ProviderQuotaError(
                        provider,
                        model,
                        policy,
                        reasons,
                        retry_after_ms,
                        {
                            "policy_id": policy["policy_id"],
                            "window_start": bucket.window_start,
                            **projected,
                        },
                        concurrent,
                    )
                entries.append((policy, bucket))
            for policy, bucket in entries:
                bucket.reserved = _add(bucket.reserved, estimate)
                self._active[policy["policy_id"]] = (
                    self._active.get(policy["policy_id"], 0) + 1
                )
            self._sequence += 1
            return ProviderQuotaReservation(
                self,
                provider,
                model,
                estimate,
                entries,
                f"quota-reservation-{self._sequence:x}",
            )

    def status(
        self,
        provider: str | None = None,
        model: str | None = None,
        *,
        now: float | None = None,
    ) -> list[dict[str, Any]]:
        current_time = self._read_clock() if now is None else float(now)
        if not math.isfinite(current_time) or current_time < 0:
            raise ValueError("provider quota status time is invalid")
        normalized_provider = (
            None
            if provider is None
            else _text("provider quota status provider", provider, 128)
        )
        normalized_model = (
            None
            if model is None
            else _optional_text("provider quota status model", model, 512)
        )
        with self._lock:
            result = []
            for policy in self._policies.values():
                if (
                    normalized_provider is not None
                    and policy["provider"] != normalized_provider
                ):
                    continue
                if model is not None and policy["model"] != normalized_model:
                    continue
                bucket = self._current_bucket(policy, current_time)
                active = self._active.get(policy["policy_id"], 0)
                result.append(
                    {
                        "schema": PROVIDER_QUOTA_SCHEMA,
                        "policy_id": policy["policy_id"],
                        "provider": policy["provider"],
                        "model": policy["model"],
                        "window_start": bucket.window_start,
                        "window_ends_at": bucket.window_start + policy["window_ms"],
                        "requests_used": bucket.used["requests"],
                        "requests_reserved": bucket.reserved["requests"],
                        "input_tokens_used": bucket.used["input_tokens"],
                        "input_tokens_reserved": bucket.reserved["input_tokens"],
                        "output_tokens_used": bucket.used["output_tokens"],
                        "output_tokens_reserved": bucket.reserved["output_tokens"],
                        "total_tokens_used": bucket.used["input_tokens"]
                        + bucket.used["output_tokens"],
                        "total_tokens_reserved": bucket.reserved["input_tokens"]
                        + bucket.reserved["output_tokens"],
                        "cost_units_used": bucket.used["cost_units"],
                        "cost_units_reserved": bucket.reserved["cost_units"],
                        "concurrent": active,
                        "next_window_at": bucket.window_start + policy["window_ms"],
                        "limits": {
                            key: policy[key]
                            for key in (
                                "max_requests",
                                "max_input_tokens",
                                "max_output_tokens",
                                "max_total_tokens",
                                "max_cost_units",
                                "max_concurrent",
                            )
                        },
                        "retention": PROVIDER_QUOTA_RETENTION,
                        "secret_material": PROVIDER_QUOTA_SECRET_MATERIAL,
                    }
                )
            return sorted(result, key=lambda item: item["policy_id"])

    def snapshot(self, *, now: float | None = None) -> dict[str, Any]:
        current_time = self._read_clock() if now is None else float(now)
        if not math.isfinite(current_time) or current_time < 0:
            raise ValueError("provider quota snapshot time is invalid")
        with self._lock:
            buckets = []
            for policy in sorted(
                self._policies.values(), key=lambda item: item["policy_id"]
            ):
                bucket = self._buckets.get(policy["policy_id"])
                if (
                    bucket is None
                    or bucket.window_start
                    != int(current_time // policy["window_ms"]) * policy["window_ms"]
                    or not any(bucket.used.values())
                ):
                    continue
                buckets.append(
                    {
                        "policy_id": policy["policy_id"],
                        "window_start": bucket.window_start,
                        "requests": bucket.used["requests"],
                        "input_tokens": bucket.used["input_tokens"],
                        "output_tokens": bucket.used["output_tokens"],
                        "cost_units": bucket.used["cost_units"],
                    }
                )
            descriptor = {
                "schema": PROVIDER_QUOTA_SNAPSHOT_SCHEMA,
                "snapshot_generation": self._generation + 1,
                "previous_snapshot_digest": None
                if self._generation == 0
                else self._previous_digest,
                "policies": self.policies(),
                "buckets": buckets,
                "retention": PROVIDER_QUOTA_RETENTION,
                "secret_material": PROVIDER_QUOTA_SECRET_MATERIAL,
            }
            snapshot = {**descriptor, "snapshot_digest": content_digest(descriptor)}
            validated = validate_provider_quota_snapshot(snapshot)
            self._generation = validated["snapshot_generation"]
            self._previous_digest = validated["snapshot_digest"]
            return validated

    def restore(self, value: Mapping[str, Any]) -> None:
        with self._lock:
            if any(count > 0 for count in self._active.values()):
                raise ProviderError(
                    "cannot restore provider quota with active reservations"
                )
            snapshot = validate_provider_quota_snapshot(value)
            policies = {policy["policy_id"]: policy for policy in snapshot["policies"]}
            buckets = {
                row["policy_id"]: _Bucket(
                    row["policy_id"],
                    row["window_start"],
                    _metrics(
                        row["requests"],
                        row["input_tokens"],
                        row["output_tokens"],
                        row["cost_units"],
                    ),
                    _metrics(),
                )
                for row in snapshot["buckets"]
            }
            self._policies = policies
            self._buckets = buckets
            self._active.clear()
            self._generation = snapshot["snapshot_generation"]
            self._previous_digest = snapshot["snapshot_digest"]

    def save(self, persistence: ProviderQuotaPersistence) -> dict[str, Any]:
        snapshot = self.snapshot()
        persistence.write(snapshot)
        return snapshot

    def restore_persisted(
        self, persistence: ProviderQuotaPersistence
    ) -> dict[str, Any] | None:
        raw = persistence.read()
        if raw is None:
            return None
        self.restore(raw)
        return validate_provider_quota_snapshot(raw)

    def _read_clock(self) -> float:
        value = float(self._clock())
        if not math.isfinite(value) or value < 0:
            raise ValueError("provider quota clock returned an invalid time")
        return value

    def _current_bucket(self, policy: Mapping[str, Any], now: float) -> _Bucket:
        start = int(now // policy["window_ms"]) * policy["window_ms"]
        existing = self._buckets.get(policy["policy_id"])
        if existing is not None and existing.window_start == start:
            return existing
        bucket = _Bucket(policy["policy_id"], start)
        self._buckets[policy["policy_id"]] = bucket
        return bucket

    def _release(self, reservation: ProviderQuotaReservation) -> None:
        with self._lock:
            for policy, bucket in reservation._entries:
                bucket.reserved = _sub(bucket.reserved, reservation._estimate)
                self._decrement(policy["policy_id"])

    def _settle(
        self, reservation: ProviderQuotaReservation, actual: Mapping[str, float | int]
    ) -> dict[str, Any]:
        over: set[str] = set()
        with self._lock:
            for policy, bucket in reservation._entries:
                bucket.reserved = _sub(bucket.reserved, reservation._estimate)
                bucket.used = _add(bucket.used, actual)
                concurrent = max(0, self._active.get(policy["policy_id"], 1) - 1)
                over.update(
                    f"{policy['policy_id']}:{reason}"
                    for reason in _over_limit(policy, bucket.used, concurrent)
                )
                self._decrement(policy["policy_id"])
        return {
            "schema": PROVIDER_QUOTA_SCHEMA,
            "reservation_id": reservation.reservation_id,
            "provider": reservation.provider,
            "model": reservation.model,
            "dispatched": reservation.is_dispatched,
            "charged_requests": actual["requests"],
            "charged_input_tokens": actual["input_tokens"],
            "charged_output_tokens": actual["output_tokens"],
            "charged_cost_units": actual["cost_units"],
            "over_limit_dimensions": sorted(over),
            "retention": PROVIDER_QUOTA_RETENTION,
            "secret_material": PROVIDER_QUOTA_SECRET_MATERIAL,
        }

    def _decrement(self, policy_id: str) -> None:
        current = self._active.get(policy_id, 0)
        if current <= 1:
            self._active.pop(policy_id, None)
        else:
            self._active[policy_id] = current - 1


class JsonProviderQuotaPersistence:
    """Canonical JSON adapter for a caller-owned quota text store."""

    def __init__(
        self,
        store: ProviderQuotaSnapshotTextStore,
        *,
        max_bytes: int = MAX_PROVIDER_QUOTA_SNAPSHOT_BYTES,
    ) -> None:
        if not callable(getattr(store, "read", None)) or not callable(
            getattr(store, "write", None)
        ):
            raise ValueError("provider quota text store is malformed")
        if (
            not isinstance(max_bytes, int)
            or isinstance(max_bytes, bool)
            or not 1 <= max_bytes <= MAX_PROVIDER_QUOTA_SNAPSHOT_BYTES
        ):
            raise ValueError(
                "provider quota persistence max_bytes is outside its bound"
            )
        self.store = store
        self.max_bytes = max_bytes

    def read(self) -> dict[str, Any] | None:
        import json

        encoded = self.store.read()
        if encoded is None:
            return None
        if (
            not isinstance(encoded, str)
            or len(encoded.encode("utf-8")) > self.max_bytes
        ):
            raise ProviderError("provider quota JSON exceeds its byte bound")
        try:
            value = json.loads(encoded)
        except (TypeError, ValueError) as error:
            raise ProviderError("provider quota JSON is invalid") from error
        if (
            json.dumps(
                value,
                ensure_ascii=False,
                separators=(",", ":"),
                sort_keys=True,
                allow_nan=False,
            )
            != encoded
        ):
            raise ProviderError("provider quota JSON is not canonical")
        return validate_provider_quota_snapshot(value)

    def write(self, snapshot: Mapping[str, Any]) -> None:
        import json

        encoded = json.dumps(
            validate_provider_quota_snapshot(snapshot),
            ensure_ascii=False,
            separators=(",", ":"),
            sort_keys=True,
            allow_nan=False,
        )
        if len(encoded.encode("utf-8")) > self.max_bytes:
            raise ProviderError("provider quota JSON exceeds its byte bound")
        self.store.write(encoded)


class TransactionalJsonProviderQuotaPersistence(JsonProviderQuotaPersistence):
    def __init__(
        self,
        store: TransactionalProviderQuotaSnapshotTextStore,
        *,
        max_bytes: int = MAX_PROVIDER_QUOTA_SNAPSHOT_BYTES,
    ) -> None:
        super().__init__(store, max_bytes=max_bytes)
        if not callable(getattr(store, "write_if_unchanged", None)):
            raise ValueError("provider quota text store lacks compare-and-swap")
        self.transactional_store = store

    def write_if_unchanged(
        self, expected_snapshot_digest: str | None, snapshot: Mapping[str, Any]
    ) -> bool:
        import json

        if expected_snapshot_digest is not None and (
            not isinstance(expected_snapshot_digest, str)
            or len(expected_snapshot_digest) != 64
        ):
            raise ProviderError("provider quota expected snapshot digest is invalid")
        encoded = json.dumps(
            validate_provider_quota_snapshot(snapshot),
            ensure_ascii=False,
            separators=(",", ":"),
            sort_keys=True,
            allow_nan=False,
        )
        return bool(
            self.transactional_store.write_if_unchanged(
                expected_snapshot_digest, encoded
            )
        )


__all__ = [
    "PROVIDER_QUOTA_SCHEMA",
    "PROVIDER_QUOTA_SNAPSHOT_SCHEMA",
    "PROVIDER_QUOTA_RETENTION",
    "PROVIDER_QUOTA_SECRET_MATERIAL",
    "MAX_PROVIDER_QUOTA_POLICIES",
    "MAX_PROVIDER_QUOTA_BUCKETS",
    "MAX_PROVIDER_QUOTA_SNAPSHOT_BYTES",
    "MAX_PROVIDER_QUOTA_WINDOW_SECONDS",
    "MAX_PROVIDER_QUOTA_METRIC",
    "MAX_PROVIDER_QUOTA_COST_UNITS",
    "MAX_PROVIDER_QUOTA_TIMESTAMP",
    "ProviderQuotaError",
    "ProviderQuotaReservation",
    "ProviderQuotaController",
    "ProviderQuotaSnapshotTextStore",
    "TransactionalProviderQuotaSnapshotTextStore",
    "ProviderQuotaPersistence",
    "JsonProviderQuotaPersistence",
    "TransactionalJsonProviderQuotaPersistence",
    "validate_provider_quota_snapshot",
]
