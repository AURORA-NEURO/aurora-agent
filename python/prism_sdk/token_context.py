"""Typed token-context planning projections.

Token counts are estimates, not measurements by default.  The Rust planner keeps the estimator
method attached to every total, refuses dry-run access to restricted candidates, checks mandatory
closure before returning a plan, and only compares requests whose non-policy fields match.  This
module mirrors those boundaries while retaining the complete wire objects for forward-compatible
token and node vocabularies.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


TOKEN_CONTEXT_MAX_TOKENS = 10_000_000
TOKEN_CONTEXT_MAX_CANDIDATES = 10_000
TOKEN_CONTEXT_MAX_INPUT_BYTES = 20_000_000
NODE_KINDS = frozenset(
    {
        "invariant",
        "evidence",
        "contradiction",
        "negative_evidence",
        "uncertainty",
        "policy_restriction",
        "summary",
        "handle",
        "attested_claim",
    }
)
RESOLUTION_DEPTHS = frozenset({"dry_run", "l0", "l1", "l2", "l3"})
ESTIMATION_METHODS = frozenset(
    {"chars_per_token4", "declared_by_caller", "provider_tokenizer", "mixed"}
)


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _unique_texts(name: str, value: Any) -> tuple[str, ...]:
    return _route_strings(name, value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    """Extract direct JSON, structured MCP content, or a REST tool envelope."""

    raw = _route_mapping("token context response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        return candidate.get("ok") is True and isinstance(candidate.get("plan"), Mapping)

    candidates: list[Mapping[str, Any]] = [raw]
    mcp = raw.get("mcp")
    if isinstance(mcp, Mapping):
        candidates.append(mcp)
        result = mcp.get("result")
        if isinstance(result, Mapping):
            candidates.append(result)
            structured = result.get("structuredContent")
            if isinstance(structured, Mapping):
                candidates.append(structured)
            content = result.get("content")
            if isinstance(content, Sequence) and not isinstance(content, (str, bytes)):
                for block in content:
                    if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                        continue
                    try:
                        decoded = json.loads(block["text"])
                    except json.JSONDecodeError as error:
                        raise ArgumentError(f"token context response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a token context plan projection")


@dataclass(frozen=True)
class TokenEstimationMethod:
    """The ruler attached to a token estimate."""

    raw: dict[str, Any]
    method: str
    provider_name: str | None = None
    mixed_methods: tuple[str, ...] = ()

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "TokenEstimationMethod":
        raw = _route_mapping("token estimation method", value)
        method = _route_text("token estimation method.method", raw.get("method"))
        if method not in ESTIMATION_METHODS:
            raise ArgumentError(f"unknown token estimation method {method!r}")
        if method == "provider_tokenizer":
            provider_name = _route_text("token estimation provider name", raw.get("name"))
            return cls(raw, method, provider_name, ())
        if method == "mixed":
            methods = _unique_texts("token estimation mixed methods", raw.get("methods"))
            if not methods:
                raise ArgumentError("mixed token estimates must name at least one method")
            return cls(raw, method, None, methods)
        return cls(raw, method)

    @property
    def measured(self) -> bool:
        return self.method == "provider_tokenizer"

    @property
    def label(self) -> str:
        if self.method == "chars_per_token4":
            return "estimated:chars-per-token-4"
        if self.method == "declared_by_caller":
            return "declared-by-caller"
        if self.method == "provider_tokenizer":
            return f"tokenizer:{self.provider_name}"
        return f"mixed:{'+'.join(self.mixed_methods)}"

    def comparable_with(self, other: "TokenEstimationMethod") -> bool:
        return (
            self.method == other.method
            and self.provider_name == other.provider_name
            and self.mixed_methods == other.mixed_methods
            and self.method != "mixed"
        )


@dataclass(frozen=True)
class TokenEstimate:
    raw: dict[str, Any]
    tokens: int
    method: TokenEstimationMethod

    @classmethod
    def from_wire(cls, value: Mapping[str, Any], *, name: str = "token estimate") -> "TokenEstimate":
        raw = _route_mapping(name, value)
        tokens = _route_count(f"{name}.tokens", raw.get("tokens"))
        if tokens > TOKEN_CONTEXT_MAX_TOKENS:
            raise ArgumentError(f"{name}.tokens must be at most {TOKEN_CONTEXT_MAX_TOKENS}")
        return cls(raw, tokens, TokenEstimationMethod.from_wire(raw.get("method")))


@dataclass(frozen=True)
class TokenContextRequest:
    """Pinned request identity and policy boundary for one context plan."""

    world_ref: str
    decision_ref: str
    role: str
    policy_id: str
    envelope_total: int
    depth: str
    compiler_version: str

    def __post_init__(self) -> None:
        for name, value in (
            ("world_ref", self.world_ref),
            ("decision_ref", self.decision_ref),
            ("role", self.role),
            ("policy_id", self.policy_id),
            ("compiler_version", self.compiler_version),
        ):
            _route_text(f"token context request.{name}", value)
        if isinstance(self.envelope_total, bool) or not isinstance(self.envelope_total, int):
            raise ArgumentError("token context request.envelope.total must be an integer")
        if not 0 <= self.envelope_total <= TOKEN_CONTEXT_MAX_TOKENS:
            raise ArgumentError(
                f"token context request.envelope.total must be between 0 and {TOKEN_CONTEXT_MAX_TOKENS}"
            )
        if not isinstance(self.depth, str) or self.depth not in RESOLUTION_DEPTHS:
            raise ArgumentError(f"unknown token context resolution depth {self.depth!r}")

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "TokenContextRequest":
        raw = _route_mapping("token context request", value)
        envelope = _route_mapping("token context request.envelope", raw.get("envelope"))
        return cls(
            _route_text("token context request.world_ref", raw.get("world_ref")),
            _route_text("token context request.decision_ref", raw.get("decision_ref")),
            _route_text("token context request.role", raw.get("role")),
            _route_text("token context request.policy_id", raw.get("policy_id")),
            _route_count("token context request.envelope.total", envelope.get("total")),
            _route_text("token context request.depth", raw.get("depth")),
            _route_text("token context request.compiler_version", raw.get("compiler_version")),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "world_ref": self.world_ref,
            "decision_ref": self.decision_ref,
            "role": self.role,
            "policy_id": self.policy_id,
            "envelope": {"total": self.envelope_total},
            "depth": self.depth,
            "compiler_version": self.compiler_version,
        }


@dataclass(frozen=True)
class TokenPlanCandidate:
    """A typed candidate offered to the planner."""

    node_id: str
    kind: str
    estimate: TokenEstimate
    mandatory: bool = False
    restricted: bool = False

    def __post_init__(self) -> None:
        _route_text("token plan candidate.node_id", self.node_id)
        if not isinstance(self.kind, str) or self.kind not in NODE_KINDS:
            raise ArgumentError(f"unknown token plan candidate kind {self.kind!r}")
        _bool("token plan candidate.mandatory", self.mandatory)
        _bool("token plan candidate.restricted", self.restricted)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any], *, index: int = 0) -> "TokenPlanCandidate":
        raw = _route_mapping(f"token plan candidates[{index}]", value)
        return cls(
            _route_text(f"token plan candidates[{index}].node_id", raw.get("node_id")),
            _route_text(f"token plan candidates[{index}].kind", raw.get("kind")),
            TokenEstimate.from_wire(raw.get("estimate"), name=f"token plan candidates[{index}].estimate"),
            raw.get("mandatory", False),
            raw.get("restricted", False),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "node_id": self.node_id,
            "kind": self.kind,
            "mandatory": self.mandatory,
            "restricted": self.restricted,
            "estimate": dict(self.estimate.raw),
        }


@dataclass(frozen=True)
class TokenContextPlanArgs:
    """Complete bounded request for token planning and optional policy comparison."""

    request: TokenContextRequest | Mapping[str, Any]
    candidates: Sequence[TokenPlanCandidate | Mapping[str, Any]]
    variant_request: TokenContextRequest | Mapping[str, Any] | None = None
    variant_candidates: Sequence[TokenPlanCandidate | Mapping[str, Any]] | None = None

    def __post_init__(self) -> None:
        request = self.request if isinstance(self.request, TokenContextRequest) else TokenContextRequest.from_wire(self.request)
        object.__setattr__(self, "request", request)
        candidates = self._normalize_candidates("candidates", self.candidates)
        object.__setattr__(self, "candidates", candidates)
        has_variant_request = self.variant_request is not None
        has_variant_candidates = self.variant_candidates is not None
        if has_variant_request != has_variant_candidates:
            raise ArgumentError("variant_request and variant_candidates must be supplied together")
        if has_variant_request:
            variant_request = self.variant_request
            if not isinstance(variant_request, TokenContextRequest):
                variant_request = TokenContextRequest.from_wire(variant_request)
            object.__setattr__(self, "variant_request", variant_request)
            object.__setattr__(
                self,
                "variant_candidates",
                self._normalize_candidates("variant_candidates", self.variant_candidates),
            )
        encoded = json.dumps(self.to_mcp_arguments(), separators=(",", ":"), ensure_ascii=False).encode("utf-8")
        if len(encoded) > TOKEN_CONTEXT_MAX_INPUT_BYTES:
            raise ArgumentError(
                f"token context input exceeds the {TOKEN_CONTEXT_MAX_INPUT_BYTES}-byte safety bound"
            )

    @staticmethod
    def _normalize_candidates(
        name: str,
        candidates: Sequence[TokenPlanCandidate | Mapping[str, Any]] | None,
    ) -> tuple[TokenPlanCandidate, ...]:
        if candidates is None or isinstance(candidates, (str, bytes)) or not isinstance(candidates, Sequence):
            raise ArgumentError(f"{name} must be an array of token plan candidates")
        if not 1 <= len(candidates) <= TOKEN_CONTEXT_MAX_CANDIDATES:
            raise ArgumentError(
                f"{name} must contain between 1 and {TOKEN_CONTEXT_MAX_CANDIDATES} candidates"
            )
        normalized = tuple(
            candidate
            if isinstance(candidate, TokenPlanCandidate)
            else TokenPlanCandidate.from_wire(candidate, index=index)
            for index, candidate in enumerate(candidates)
        )
        ids = tuple(candidate.node_id for candidate in normalized)
        if len(ids) != len(set(ids)):
            raise ArgumentError(f"{name} must not contain duplicate node_id values")
        return normalized

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "TokenContextPlanArgs":
        raw = _route_mapping("token context plan arguments", value)
        candidates = _array("token context plan candidates", raw.get("candidates"))
        variant_raw = raw.get("variant_candidates")
        return cls(
            raw.get("request"),
            candidates,
            raw.get("variant_request"),
            None if variant_raw is None else _array("token context plan variant_candidates", variant_raw),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "request": self.request.to_dict(),
            "candidates": [candidate.to_dict() for candidate in self.candidates],
        }
        if self.variant_request is not None:
            result["variant_request"] = self.variant_request.to_dict()
            result["variant_candidates"] = [candidate.to_dict() for candidate in self.variant_candidates or ()]
        return result


@dataclass(frozen=True)
class TokenContextPlanReport:
    raw: dict[str, Any]
    request_digest: str
    plan_digest: str
    candidates: tuple[str, ...]
    mandatory: tuple[str, ...]
    handles: tuple[str, ...]
    mandatory_estimate: TokenEstimate
    optional_estimate: TokenEstimate
    envelope_total: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "TokenContextPlanReport":
        raw = _route_mapping("token context plan", value)
        candidates = _unique_texts("token context plan candidates", raw.get("candidates"))
        mandatory = _unique_texts("token context plan mandatory", raw.get("mandatory"))
        handles = _unique_texts("token context plan handles", raw.get("handles"))
        candidate_set = set(candidates)
        if not set(mandatory).issubset(candidate_set):
            raise ArgumentError("token context mandatory closure contains an unknown candidate")
        if not set(handles).issubset(candidate_set):
            raise ArgumentError("token context handles contain an unknown candidate")
        envelope = _route_mapping("token context plan envelope", raw.get("envelope"))
        envelope_total = _route_count("token context plan envelope.total", envelope.get("total"))
        mandatory_estimate = TokenEstimate.from_wire(
            raw.get("mandatory_estimate"), name="token context mandatory_estimate"
        )
        if mandatory_estimate.tokens > envelope_total:
            raise ArgumentError("token context mandatory estimate exceeds its envelope")
        return cls(
            raw,
            _route_text("token context request_digest", raw.get("request_digest")),
            _route_text("token context plan_digest", raw.get("plan_digest")),
            candidates,
            mandatory,
            handles,
            mandatory_estimate,
            TokenEstimate.from_wire(raw.get("optional_estimate"), name="token context optional_estimate"),
            envelope_total,
        )

    @property
    def discretionary_tokens(self) -> int:
        return self.envelope_total - self.mandatory_estimate.tokens

    @property
    def is_dry_run_projection(self) -> bool:
        return bool(self.handles)


@dataclass(frozen=True)
class TokenPolicyComparisonReport:
    raw: dict[str, Any]
    comparison_id: str
    mode: str
    baseline_policy: str
    variant_policy: str
    baseline_plan: TokenContextPlanReport
    variant_plan: TokenContextPlanReport

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "TokenPolicyComparisonReport":
        raw = _route_mapping("token policy comparison", value)
        mode = _route_text("token policy comparison mode", raw.get("mode"))
        if mode != "policy_only":
            raise ArgumentError("token policy comparison mode must be policy_only")
        baseline = TokenContextPlanReport.from_wire(raw.get("baseline_plan"))
        variant = TokenContextPlanReport.from_wire(raw.get("variant_plan"))
        return cls(
            raw,
            _route_text("token policy comparison id", raw.get("comparison_id")),
            mode,
            _route_text("token policy baseline_policy", raw.get("baseline_policy")),
            _route_text("token policy variant_policy", raw.get("variant_policy")),
            baseline,
            variant,
        )

    @property
    def mandatory_difference(self) -> int | None:
        if not self.baseline_plan.mandatory_estimate.method.comparable_with(
            self.variant_plan.mandatory_estimate.method
        ):
            return None
        return self.variant_plan.mandatory_estimate.tokens - self.baseline_plan.mandatory_estimate.tokens

    @property
    def mandatory_added(self) -> tuple[str, ...]:
        return tuple(sorted(set(self.variant_plan.mandatory) - set(self.baseline_plan.mandatory)))

    @property
    def mandatory_removed(self) -> tuple[str, ...]:
        return tuple(sorted(set(self.baseline_plan.mandatory) - set(self.variant_plan.mandatory)))

    @property
    def estimates_comparable(self) -> bool:
        return self.mandatory_difference is not None


@dataclass(frozen=True)
class TokenContextPlanningReport:
    """Complete fail-closed token planning projection."""

    raw: dict[str, Any]
    ok: bool
    plan: TokenContextPlanReport
    comparison: TokenPolicyComparisonReport | None
    guarantees: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "TokenContextPlanningReport":
        raw = _payload(value)
        ok = _bool("token context planning ok", raw.get("ok"))
        if not ok:
            raise ArgumentError("token context planning projection must be successful")
        comparison_raw = raw.get("comparison")
        comparison = (
            None
            if comparison_raw is None
            else TokenPolicyComparisonReport.from_wire(comparison_raw)
        )
        return cls(
            raw,
            ok,
            TokenContextPlanReport.from_wire(raw.get("plan")),
            comparison,
            _route_strings("token context guarantees", raw.get("guarantees", [])),
        )

    @property
    def has_comparison(self) -> bool:
        return self.comparison is not None

    @property
    def mandatory_closure_affordable(self) -> bool:
        return self.plan.mandatory_estimate.tokens <= self.plan.envelope_total

    @property
    def estimates_are_measured(self) -> bool:
        return self.plan.mandatory_estimate.method.measured and self.plan.optional_estimate.method.measured

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def token_context_plan_report(value: Mapping[str, Any]) -> TokenContextPlanningReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return TokenContextPlanningReport.from_wire(value)


__all__ = [
    "TOKEN_CONTEXT_MAX_TOKENS",
    "TOKEN_CONTEXT_MAX_CANDIDATES",
    "TOKEN_CONTEXT_MAX_INPUT_BYTES",
    "NODE_KINDS",
    "RESOLUTION_DEPTHS",
    "ESTIMATION_METHODS",
    "TokenEstimationMethod",
    "TokenEstimate",
    "TokenContextRequest",
    "TokenPlanCandidate",
    "TokenContextPlanArgs",
    "TokenContextPlanReport",
    "TokenPolicyComparisonReport",
    "TokenContextPlanningReport",
    "token_context_plan_report",
]
