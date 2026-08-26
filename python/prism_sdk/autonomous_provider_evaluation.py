"""Provider-receipt evaluation and model-arm learning.

Provider transport is deliberately not a quality signal.  This module turns the redacted
provider invocation receipt into a bounded evaluator projection, accepts an independent
caller-owned quality judgment, and optionally applies that judgment to a value-only model
bandit state.  Prompts, responses, messages, request bodies, credentials, and arbitrary
provider payloads are rejected at the boundary rather than merely omitted from the output.

The context digest follows the existing contextual learner contract: the four normalized fields
are encoded in their stable order and hashed without JSON key sorting.  This is important for
restart/replay parity with the Rust, TypeScript, and Python selection paths.
"""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import math
from typing import Any, Callable, Mapping, Sequence

from .authoring import canonical_bytes, content_digest
from .autonomy_provider import AUTONOMOUS_PROVIDER_INVOCATION_SCHEMA, AutonomousProviderInvocationReceipt
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError


AUTONOMOUS_PROVIDER_EVALUATION_SCHEMA = "bioprism-python-autonomous-provider-evaluation/0.1"
AUTONOMOUS_PROVIDER_LEARNING_SCHEMA = "bioprism-python-autonomous-provider-learning/0.1"
MAX_AUTONOMOUS_PROVIDER_EVALUATION_EVIDENCE_BYTES = 256_000
MAX_AUTONOMOUS_PROVIDER_EVALUATION_RECEIPTS = 256
_PROVIDER_STATUSES = frozenset({"completed", "provider_refused"})
_PROVIDER_OUTCOMES = frozenset({"success", "failure"})
_SAFE_IDENTIFIER = frozenset("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.-")
_SHA256 = frozenset("0123456789abcdef")
_FORBIDDEN_FIELDS = frozenset(
    {
        "apikey", "authorization", "bearer", "credential", "password", "secret",
        "accesstoken", "refreshtoken", "token", "privatekey", "prompt", "response",
        "rawpayload", "arguments", "output", "task", "messages", "headers", "body",
    }
)
_RECEIPT_FIELDS = frozenset(
    {
        "schema", "execution_id", "provider", "model", "kind", "attempt", "turn", "status",
        "outcome", "input_tokens", "output_tokens", "estimated_cost_units", "actual_cost_units",
        "latency_ms", "selection_digest", "outcome_digest", "request_id_digest", "failure_class",
        "status_code", "retention", "secret_material",
    }
)


def _bytes(value: str) -> int:
    return len(value.encode("utf-8"))


def _text(name: str, value: Any, maximum: int = 512) -> str:
    if not isinstance(value, str) or not value or "\x00" in value or _bytes(value) > maximum:
        raise ArgumentError(f"{name} is outside its bounded text contract")
    return value


def _identifier(name: str, value: Any, maximum: int = 256) -> str:
    value = _text(name, value, maximum)
    if any(character not in _SAFE_IDENTIFIER for character in value):
        raise ArgumentError(f"{name} must be a bounded identifier")
    return value


def _optional_identifier(name: str, value: Any) -> str | None:
    return None if value is None else _identifier(name, value)


def _digest(name: str, value: Any, *, optional: bool = False) -> str | None:
    if value is None:
        if optional:
            return None
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    if not isinstance(value, str) or len(value) != 64 or any(character not in _SHA256 for character in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _integer(name: str, value: Any, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= maximum:
        raise ArgumentError(f"{name} must be a non-negative integer within its bound")
    return value


def _number(name: str, value: Any, maximum: float) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)) or not 0 <= float(value) <= maximum:
        raise ArgumentError(f"{name} must be finite and within its bound")
    return float(value)


def _assert_safe(value: Any, *, depth: int = 0) -> None:
    if depth > 32:
        raise ArgumentError("provider evaluator evidence is too deeply nested")
    if isinstance(value, Mapping):
        if len(value) > 4096:
            raise ArgumentError("provider evaluator evidence contains too many object keys")
        for key, child in value.items():
            normalized = "".join(character for character in key.lower() if character.isalnum()) if isinstance(key, str) else ""
            if normalized in _FORBIDDEN_FIELDS:
                raise ArgumentError("provider evaluator evidence contains transient or secret-shaped fields")
            _assert_safe(child, depth=depth + 1)
    elif isinstance(value, (list, tuple)):
        if len(value) > 4096:
            raise ArgumentError("provider evaluator evidence contains too many array items")
        for child in value:
            _assert_safe(child, depth=depth + 1)
    elif isinstance(value, float) and not math.isfinite(value):
        raise ArgumentError("provider evaluator evidence contains a non-finite number")


def _safe_json(name: str, value: Any, *, maximum: int = MAX_AUTONOMOUS_PROVIDER_EVALUATION_EVIDENCE_BYTES) -> Any:
    _assert_safe(value)
    try:
        encoded = canonical_bytes(value)
    except (TypeError, ValueError, ArgumentError) as error:
        raise ArgumentError(f"{name} must be canonical JSON") from error
    if len(encoded) > maximum:
        raise ArgumentError(f"{name} exceeds its bounded size")
    return json.loads(encoded.decode("utf-8"))


def _ordered_digest(value: Mapping[str, Any]) -> str:
    try:
        encoded = json.dumps(value, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise ArgumentError("provider learning context must be JSON-safe") from error
    return hashlib.sha256(encoded).hexdigest()


def _receipt_mapping(receipt: AutonomousProviderInvocationReceipt | Mapping[str, Any]) -> Mapping[str, Any]:
    if isinstance(receipt, AutonomousProviderInvocationReceipt):
        return receipt.to_dict()
    if not isinstance(receipt, Mapping):
        raise ArgumentError("provider evaluation requires an AutonomousProviderInvocationReceipt or mapping")
    if set(receipt).difference(_RECEIPT_FIELDS):
        raise ArgumentError("provider invocation receipt contains unsupported fields")
    return receipt


def _receipt_metadata(receipt: AutonomousProviderInvocationReceipt | Mapping[str, Any]) -> dict[str, Any]:
    value = _receipt_mapping(receipt)
    if value.get("schema") != AUTONOMOUS_PROVIDER_INVOCATION_SCHEMA:
        raise ArgumentError("provider evaluation requires an autonomous provider invocation receipt")
    status = value.get("status")
    outcome = value.get("outcome")
    if status not in _PROVIDER_STATUSES or outcome not in _PROVIDER_OUTCOMES:
        raise ArgumentError("provider invocation receipt status or outcome is invalid")
    if (status == "completed") != (outcome == "success"):
        raise ArgumentError("provider invocation receipt status and outcome disagree")
    status_code = value.get("status_code")
    if status_code is not None and (isinstance(status_code, bool) or not isinstance(status_code, int) or not 100 <= status_code <= 599):
        raise ArgumentError("provider invocation receipt status_code is invalid")
    return {
        "execution_id": _optional_identifier("provider receipt execution_id", value.get("execution_id")),
        "provider": _identifier("provider receipt provider", value.get("provider")),
        "model": _identifier("provider receipt model", value.get("model")),
        "kind": _identifier("provider receipt kind", value.get("kind")),
        "attempt": _integer("provider receipt attempt", value.get("attempt"), 64),
        "turn": _integer("provider receipt turn", value.get("turn"), 256),
        "status": status,
        "outcome": outcome,
        "input_tokens": _integer("provider receipt input_tokens", value.get("input_tokens"), 1_000_000_000),
        "output_tokens": _integer("provider receipt output_tokens", value.get("output_tokens"), 1_000_000_000),
        "estimated_cost_units": _number("provider receipt estimated_cost_units", value.get("estimated_cost_units"), 1_000_000_000),
        "actual_cost_units": _number("provider receipt actual_cost_units", value.get("actual_cost_units"), 1_000_000_000),
        "latency_ms": _number("provider receipt latency_ms", value.get("latency_ms"), 86_400_000),
        "selection_digest": _digest("provider receipt selection_digest", value.get("selection_digest"), optional=True),
        "outcome_digest": _digest("provider receipt outcome_digest", value.get("outcome_digest")),
        "request_id_digest": _digest("provider receipt request_id_digest", value.get("request_id_digest"), optional=True),
        "failure_class": _optional_identifier("provider receipt failure_class", value.get("failure_class")),
        "status_code": status_code,
    }


def autonomous_provider_receipt_identity(receipt: AutonomousProviderInvocationReceipt | Mapping[str, Any]) -> str:
    """Return the stable receipt identity used for evidence maps and replay protection."""

    metadata = _receipt_metadata(receipt)
    return f"{metadata['execution_id'] or 'unjournaled'}:{metadata['provider']}/{metadata['model']}:{metadata['attempt']}:{metadata['turn']}:{metadata['outcome_digest']}"


@dataclass(frozen=True, slots=True)
class AutonomousProviderOutcomeContext:
    domain: str
    capability: str
    risk_class: str
    task_family: str | None = None
    contract_digest: str | None = None
    context_digest: str | None = None

    def __post_init__(self) -> None:
        if self.domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("provider evaluation context domain is unsupported")
        _identifier("provider evaluation context capability", self.capability)
        _identifier("provider evaluation context risk_class", self.risk_class)
        if self.task_family is not None:
            _identifier("provider evaluation context task_family", self.task_family)
        if self.contract_digest is not None:
            _digest("provider evaluation context contract_digest", self.contract_digest)
        if self.context_digest is not None:
            _digest("provider evaluation context context_digest", self.context_digest)

    def stable(self) -> dict[str, Any]:
        return {
            "domain": self.domain,
            "capability": self.capability,
            "risk_class": self.risk_class,
            "task_family": self.task_family,
        }

    def to_dict(self) -> dict[str, Any]:
        result = self.stable()
        result["contract_digest"] = self.contract_digest
        result["context_digest"] = self.context_digest
        return result


def _normalize_context(value: AutonomousProviderOutcomeContext | Mapping[str, Any] | None) -> tuple[AutonomousProviderOutcomeContext, dict[str, Any], str | None]:
    if value is None:
        default = AutonomousProviderOutcomeContext("cross_domain", "provider_invocation", "provider_call")
        return default, default.stable(), None
    if isinstance(value, AutonomousProviderOutcomeContext):
        context_value = value.to_dict()
    elif isinstance(value, Mapping):
        context_value = value
    else:
        raise ArgumentError("provider evaluation context must be a mapping or AutonomousProviderOutcomeContext")
    allowed = {"domain", "capability", "risk_class", "task_family", "contract_digest", "context_digest"}
    if set(context_value).difference(allowed):
        raise ArgumentError("provider evaluation context contains unsupported fields")
    context = AutonomousProviderOutcomeContext(
        domain=context_value.get("domain"),
        capability=context_value.get("capability"),
        risk_class=context_value.get("risk_class"),
        task_family=context_value.get("task_family"),
        contract_digest=context_value.get("contract_digest"),
        context_digest=context_value.get("context_digest"),
    )
    expected = _ordered_digest(context.stable())
    if context.context_digest is not None and context.context_digest != expected:
        raise ArgumentError("provider evaluation context_digest does not match its context")
    # An explicitly supplied context is always bound, even if its digest was omitted by the
    # caller.  This makes accidental global credit impossible when context was requested.
    return AutonomousProviderOutcomeContext(
        context.domain, context.capability, context.risk_class, context.task_family,
        context.contract_digest, expected,
    ), context.stable(), expected


@dataclass(frozen=True, slots=True)
class AutonomousProviderOutcomeEvaluationInput:
    schema: str
    receipt_identity: str
    execution_id: str | None
    provider: str
    model: str
    kind: str
    attempt: int
    turn: int
    status: str
    outcome: str
    input_tokens: int
    output_tokens: int
    estimated_cost_units: float
    actual_cost_units: float
    latency_ms: float
    selection_digest: str | None
    outcome_digest: str
    request_id_digest: str | None
    failure_class: str | None
    status_code: int | None
    domain: str
    capability: str
    risk_class: str
    task_family: str | None
    contract_digest: str | None
    context_digest: str | None
    context: Mapping[str, Any]
    evidence_digest: str
    evidence: Mapping[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": self.schema,
            "receipt_identity": self.receipt_identity,
            "execution_id": self.execution_id,
            "provider": self.provider,
            "model": self.model,
            "kind": self.kind,
            "attempt": self.attempt,
            "turn": self.turn,
            "status": self.status,
            "outcome": self.outcome,
            "input_tokens": self.input_tokens,
            "output_tokens": self.output_tokens,
            "estimated_cost_units": self.estimated_cost_units,
            "actual_cost_units": self.actual_cost_units,
            "latency_ms": self.latency_ms,
            "selection_digest": self.selection_digest,
            "outcome_digest": self.outcome_digest,
            "request_id_digest": self.request_id_digest,
            "failure_class": self.failure_class,
            "status_code": self.status_code,
            "domain": self.domain,
            "capability": self.capability,
            "risk_class": self.risk_class,
            "task_family": self.task_family,
            "contract_digest": self.contract_digest,
            "context_digest": self.context_digest,
            "context": dict(self.context),
            "evidence_digest": self.evidence_digest,
            "evidence": dict(self.evidence),
            "retention": "digests_and_safe_evidence_only_no_provider_payloads_or_credentials",
        }


def autonomous_provider_outcome_evaluation_input(
    receipt: AutonomousProviderInvocationReceipt | Mapping[str, Any],
    *,
    context: AutonomousProviderOutcomeContext | Mapping[str, Any] | None = None,
    evidence: Mapping[str, Any] | None = None,
) -> AutonomousProviderOutcomeEvaluationInput:
    """Project one receipt into the safe callback input contract."""

    metadata = _receipt_metadata(receipt)
    normalized_context, stable_context, context_digest = _normalize_context(context)
    if evidence is None:
        evidence = {}
    if not isinstance(evidence, Mapping):
        raise ArgumentError("provider evaluator evidence must be a mapping")
    safe_evidence = _safe_json("provider evaluator evidence", dict(evidence))
    identity = autonomous_provider_receipt_identity(receipt)
    return AutonomousProviderOutcomeEvaluationInput(
        schema=AUTONOMOUS_PROVIDER_EVALUATION_SCHEMA,
        receipt_identity=identity,
        **metadata,
        domain=normalized_context.domain,
        capability=normalized_context.capability,
        risk_class=normalized_context.risk_class,
        task_family=normalized_context.task_family,
        contract_digest=normalized_context.contract_digest,
        context_digest=context_digest,
        context=stable_context,
        evidence_digest=content_digest(safe_evidence),
        evidence=safe_evidence,
    )


@dataclass(frozen=True, slots=True)
class AutonomousProviderEvaluatorAssessment:
    reward: float
    passed: bool
    failed: bool | None = None
    feedback_digest: str | None = None
    failure_class: str | None = None
    evidence_digest: str | None = None


@dataclass(frozen=True, slots=True)
class AutonomousProviderEvaluation:
    receipt_identity: str
    execution_id: str | None
    domain: str
    capability: str
    risk_class: str
    contract_digest: str | None
    context_digest: str | None
    provider: str
    model: str
    arm_id: str
    status: str
    outcome: str
    attempt: int
    turn: int
    evidence_digest: str
    decision_digest: str
    evaluator_id: str
    evaluator_version: str
    reward: float
    passed: bool
    failed: bool
    feedback_digest: str | None
    failure_class: str | None
    model_outcome_digest: str
    idempotent_replay: bool

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_PROVIDER_EVALUATION_SCHEMA,
            "receipt_identity": self.receipt_identity,
            "execution_id": self.execution_id,
            "domain": self.domain,
            "capability": self.capability,
            "risk_class": self.risk_class,
            "contract_digest": self.contract_digest,
            "context_digest": self.context_digest,
            "provider": self.provider,
            "model": self.model,
            "arm_id": self.arm_id,
            "status": self.status,
            "outcome": self.outcome,
            "attempt": self.attempt,
            "turn": self.turn,
            "evidence_digest": self.evidence_digest,
            "decision_digest": self.decision_digest,
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "reward": self.reward,
            "passed": self.passed,
            "failed": self.failed,
            "feedback_digest": self.feedback_digest,
            "failure_class": self.failure_class,
            "model_outcome_digest": self.model_outcome_digest,
            "idempotent_replay": self.idempotent_replay,
            "retention": "value_only",
        }


@dataclass(frozen=True, slots=True)
class AutonomousProviderLearningReport:
    status: str
    receipts: int
    evaluations: tuple[Mapping[str, Any], ...]
    by_domain: Mapping[str, int]
    by_status: Mapping[str, int]
    by_model: Mapping[str, int]
    next_learning_state: Mapping[str, Any] | None
    next_learning_state_digest: str | None
    learning_digest: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_PROVIDER_LEARNING_SCHEMA,
            "status": self.status,
            "receipts": self.receipts,
            "evaluations": [dict(item) for item in self.evaluations],
            "by_domain": dict(self.by_domain),
            "by_status": dict(self.by_status),
            "by_model": dict(self.by_model),
            "next_learning_state": None if self.next_learning_state is None else dict(self.next_learning_state),
            "next_learning_state_digest": self.next_learning_state_digest,
            "learning_digest": self.learning_digest,
            "retention": "metadata_and_digests_only",
            "secret_material": "never_returned",
        }


def _normalize_assessment(raw: Mapping[str, Any] | AutonomousProviderEvaluatorAssessment, *, evaluator_id: str, evaluator_version: str, evidence_digest: str) -> dict[str, Any]:
    if isinstance(raw, AutonomousProviderEvaluatorAssessment):
        value = {
            "reward": raw.reward,
            "passed": raw.passed,
            "failed": raw.failed,
            "feedback_digest": raw.feedback_digest,
            "failure_class": raw.failure_class,
            "evidence_digest": raw.evidence_digest,
        }
    elif isinstance(raw, Mapping):
        value = dict(raw)
    else:
        raise ArgumentError("provider evaluator callback must return a mapping or assessment")
    allowed = {"reward", "passed", "failed", "feedback_digest", "failure_class", "evidence_digest"}
    if set(value).difference(allowed):
        raise ArgumentError("provider evaluator decision contains unsupported fields")
    reward = value.get("reward")
    if isinstance(reward, bool) or not isinstance(reward, (int, float)) or not math.isfinite(float(reward)) or not -1 <= float(reward) <= 1:
        raise ArgumentError("provider evaluator reward must be finite and within [-1, 1]")
    passed = value.get("passed")
    if not isinstance(passed, bool):
        raise ArgumentError("provider evaluator passed must be boolean")
    failed = not passed if value.get("failed") is None else value.get("failed")
    if not isinstance(failed, bool) or (passed and failed):
        raise ArgumentError("provider evaluator passed and failed are inconsistent")
    feedback_digest = _digest("provider evaluator feedback_digest", value.get("feedback_digest"), optional=True)
    failure_class = _optional_identifier("provider evaluator failure_class", value.get("failure_class"))
    returned_evidence_digest = _digest("provider evaluator evidence_digest", value.get("evidence_digest"), optional=True)
    if returned_evidence_digest is not None and returned_evidence_digest != evidence_digest:
        raise ArgumentError("provider evaluator evidence_digest does not match the input")
    return {
        "evaluator_id": evaluator_id,
        "evaluator_version": evaluator_version,
        "reward": float(reward),
        "passed": passed,
        "failed": failed,
        "feedback_digest": feedback_digest,
        "failure_class": failure_class,
        "evidence_digest": evidence_digest,
    }


def _prior_credit(state: Mapping[str, Any] | None, outcome_digest: str) -> Mapping[str, Any] | None:
    if not isinstance(state, Mapping) or not isinstance(state.get("credited_outcomes"), Sequence):
        return None
    return next((item for item in state["credited_outcomes"] if isinstance(item, Mapping) and item.get("outcome_digest") == outcome_digest), None)


class AutonomousProviderOutcomeEvaluator:
    """Evaluate provider receipts without allowing transport status to create reward."""

    def __init__(
        self,
        evaluator: Callable[[Mapping[str, Any]], Mapping[str, Any] | AutonomousProviderEvaluatorAssessment],
        *,
        evaluator_id: str,
        evaluator_version: str,
    ) -> None:
        if not callable(evaluator):
            raise ArgumentError("provider evaluator must be callable")
        self.evaluator = evaluator
        self.evaluator_id = _identifier("provider evaluator_id", evaluator_id)
        self.evaluator_version = _identifier("provider evaluator_version", evaluator_version)

    def assess(self, outcome: AutonomousProviderOutcomeEvaluationInput) -> AutonomousProviderEvaluation:
        if not isinstance(outcome, AutonomousProviderOutcomeEvaluationInput) or outcome.schema != AUTONOMOUS_PROVIDER_EVALUATION_SCHEMA:
            raise ArgumentError("provider evaluator input schema is invalid")
        # Rebuild the projection from its safe fields before invoking the callback.  This keeps
        # the public evaluator seam resistant to a caller mutating a dataclass through an unsafe
        # object wrapper or passing a projection with a forged identity digest.
        rebuilt = autonomous_provider_outcome_evaluation_input(
            {
                "schema": AUTONOMOUS_PROVIDER_INVOCATION_SCHEMA,
                "execution_id": outcome.execution_id,
                "provider": outcome.provider,
                "model": outcome.model,
                "kind": outcome.kind,
                "attempt": outcome.attempt,
                "turn": outcome.turn,
                "status": outcome.status,
                "outcome": outcome.outcome,
                "input_tokens": outcome.input_tokens,
                "output_tokens": outcome.output_tokens,
                "estimated_cost_units": outcome.estimated_cost_units,
                "actual_cost_units": outcome.actual_cost_units,
                "latency_ms": outcome.latency_ms,
                "selection_digest": outcome.selection_digest,
                "outcome_digest": outcome.outcome_digest,
                "request_id_digest": outcome.request_id_digest,
                "failure_class": outcome.failure_class,
                "status_code": outcome.status_code,
            },
            context=None if outcome.context_digest is None else {
                "domain": outcome.domain,
                "capability": outcome.capability,
                "risk_class": outcome.risk_class,
                "task_family": outcome.task_family,
                "contract_digest": outcome.contract_digest,
                "context_digest": outcome.context_digest,
            },
            evidence=dict(outcome.evidence),
        )
        if rebuilt.receipt_identity != outcome.receipt_identity or rebuilt.evidence_digest != outcome.evidence_digest:
            raise ArgumentError("provider evaluator input identity is invalid")
        try:
            raw = self.evaluator(rebuilt.to_dict())
        except Exception as error:
            raise ArgumentError("provider evaluator callback failed") from error
        decision = _normalize_assessment(raw, evaluator_id=self.evaluator_id, evaluator_version=self.evaluator_version, evidence_digest=outcome.evidence_digest)
        base = {
            "schema": AUTONOMOUS_PROVIDER_EVALUATION_SCHEMA,
            "receipt_identity": outcome.receipt_identity,
            "execution_id": outcome.execution_id,
            "domain": outcome.domain,
            "capability": outcome.capability,
            "risk_class": outcome.risk_class,
            "contract_digest": outcome.contract_digest,
            "context_digest": outcome.context_digest,
            "provider": outcome.provider,
            "model": outcome.model,
            "arm_id": f"{outcome.provider}/{outcome.model}",
            "status": outcome.status,
            "outcome": outcome.outcome,
            "attempt": outcome.attempt,
            "turn": outcome.turn,
            "evidence_digest": outcome.evidence_digest,
            **decision,
            "retention": "value_only",
        }
        decision_digest = content_digest(base)
        metadata_input = dict(rebuilt.to_dict())
        metadata_input.pop("evidence", None)
        model_outcome_digest = content_digest(
            {
                "schema": "bioprism-autonomous-provider-model-outcome/0.1",
                "receipt_identity": outcome.receipt_identity,
                "input_digest": content_digest(metadata_input),
                "decision_digest": decision_digest,
            }
        )
        return AutonomousProviderEvaluation(
            receipt_identity=outcome.receipt_identity,
            execution_id=outcome.execution_id,
            domain=outcome.domain,
            capability=outcome.capability,
            risk_class=outcome.risk_class,
            contract_digest=outcome.contract_digest,
            context_digest=outcome.context_digest,
            provider=outcome.provider,
            model=outcome.model,
            arm_id=base["arm_id"],
            status=outcome.status,
            outcome=outcome.outcome,
            attempt=outcome.attempt,
            turn=outcome.turn,
            evidence_digest=outcome.evidence_digest,
            decision_digest=decision_digest,
            evaluator_id=self.evaluator_id,
            evaluator_version=self.evaluator_version,
            reward=decision["reward"],
            passed=decision["passed"],
            failed=decision["failed"],
            feedback_digest=decision["feedback_digest"],
            failure_class=decision["failure_class"],
            model_outcome_digest=model_outcome_digest,
            idempotent_replay=False,
        )

    def evaluate_receipts(
        self,
        receipts: Sequence[AutonomousProviderInvocationReceipt | Mapping[str, Any]],
        *,
        contexts: Mapping[str, AutonomousProviderOutcomeContext | Mapping[str, Any]] | None = None,
        evidence: Mapping[str, Mapping[str, Any]] | None = None,
        learning_state: Mapping[str, Any] | None = None,
        learning_updater: Callable[[Mapping[str, Any], Mapping[str, Any], Mapping[str, Any]], Mapping[str, Any]] | None = None,
    ) -> AutonomousProviderLearningReport:
        if not isinstance(receipts, Sequence) or isinstance(receipts, (str, bytes)) or len(receipts) > MAX_AUTONOMOUS_PROVIDER_EVALUATION_RECEIPTS:
            raise ArgumentError(f"provider receipt batches must contain at most {MAX_AUTONOMOUS_PROVIDER_EVALUATION_RECEIPTS} entries")
        identities = [autonomous_provider_receipt_identity(receipt) for receipt in receipts]
        if len(set(identities)) != len(identities):
            raise ArgumentError("provider receipt batches cannot contain duplicate identities")
        metadata = [_receipt_metadata(receipt) for receipt in receipts]
        outcome_counts: dict[str, int] = {}
        for item in metadata:
            outcome_counts[item["outcome_digest"]] = outcome_counts.get(item["outcome_digest"], 0) + 1
        unique_outcomes = {digest for digest, count in outcome_counts.items() if count == 1}
        evidence_map = {} if evidence is None else dict(evidence)
        context_map = {} if contexts is None else dict(contexts)
        for name, values in (("provider receipt evidence", evidence_map), ("provider receipt contexts", context_map)):
            if not isinstance(values, Mapping):
                raise ArgumentError(f"{name} must be a mapping")
            if set(values).difference(set(identities) | unique_outcomes):
                raise ArgumentError(f"{name} contains an unknown receipt identity")
        state: Mapping[str, Any] | None = None if learning_state is None else _safe_json("provider learning state", dict(learning_state))
        evaluations: list[Mapping[str, Any]] = []
        by_domain: dict[str, int] = {}
        by_status: dict[str, int] = {}
        by_model: dict[str, int] = {}
        for receipt, identity, item in zip(receipts, identities, metadata):
            packet = evidence_map.get(identity)
            if packet is None and item["outcome_digest"] in unique_outcomes:
                packet = evidence_map.get(item["outcome_digest"])
            if packet is None:
                packet = {}
            if not isinstance(packet, Mapping):
                raise ArgumentError("provider receipt evidence values must be mappings")
            scoped_context = context_map.get(identity)
            if scoped_context is None and item["outcome_digest"] in unique_outcomes:
                scoped_context = context_map.get(item["outcome_digest"])
            input_value = autonomous_provider_outcome_evaluation_input(receipt, context=scoped_context, evidence=packet)
            decision = self.assess(input_value)
            replay = _prior_credit(state, decision.model_outcome_digest) is not None
            update = {
                "arm_id": decision.arm_id,
                "reward": decision.reward,
                "failed": decision.failed,
                "outcome_digest": decision.model_outcome_digest,
                "contract_digest": input_value.contract_digest or input_value.selection_digest,
                "context_digest": input_value.context_digest,
                "context": dict(input_value.context),
                "latency_ms": input_value.latency_ms,
                "provider": input_value.provider,
                "model": input_value.model,
            }
            if input_value.context_digest is None:
                update.pop("context", None)
            learning_update = "not_configured"
            if learning_updater is not None:
                if not callable(learning_updater):
                    raise ArgumentError("provider learning updater must be callable")
                try:
                    updated = learning_updater(dict(state or {}), {
                        "evaluator_id": decision.evaluator_id,
                        "evaluator_version": decision.evaluator_version,
                        "reward": decision.reward,
                        "passed": decision.passed,
                        "failed": decision.failed,
                        "feedback_digest": decision.feedback_digest,
                        "failure_class": decision.failure_class,
                        "evidence_digest": decision.evidence_digest,
                    }, update)
                except Exception as error:
                    raise ArgumentError("provider learning updater failed") from error
                if not isinstance(updated, Mapping):
                    raise ArgumentError("provider learning updater must return a mapping")
                state = _safe_json("next provider learning state", dict(updated))
                learning_update = "applied"
            evaluation = decision.to_dict()
            evaluation["idempotent_replay"] = replay
            evaluation["learning_update"] = learning_update
            evaluations.append(evaluation)
            by_domain[input_value.domain] = by_domain.get(input_value.domain, 0) + 1
            by_status[input_value.status] = by_status.get(input_value.status, 0) + 1
            by_model[decision.arm_id] = by_model.get(decision.arm_id, 0) + 1
        digest_values = [{key: value for key, value in item.items() if key not in {"idempotent_replay", "learning_update"}} for item in evaluations]
        learning_digest = content_digest(digest_values)
        return AutonomousProviderLearningReport(
            status="completed" if evaluations else "no_receipts",
            receipts=len(evaluations),
            evaluations=tuple(evaluations),
            by_domain=dict(sorted(by_domain.items())),
            by_status=dict(sorted(by_status.items())),
            by_model=dict(sorted(by_model.items())),
            next_learning_state=None if state is None else dict(state),
            next_learning_state_digest=None if state is None else content_digest(state),
            learning_digest=learning_digest,
        )


def _state(value: Mapping[str, Any] | None) -> dict[str, Any]:
    if value is None:
        return {"schema": "bioprism-brain-bandit/0.1", "generation": 0, "arms": [], "credited_outcomes": [], "contextual_states": []}
    normalized = _safe_json("provider learning state", dict(value))
    if normalized.get("schema") not in (None, "bioprism-brain-bandit/0.1"):
        raise ArgumentError("provider learning state schema is unsupported")
    normalized["schema"] = "bioprism-brain-bandit/0.1"
    generation = normalized.get("generation", 0)
    if isinstance(generation, bool) or not isinstance(generation, int) or generation < 0:
        raise ArgumentError("provider learning state generation is malformed")
    for field in ("arms", "credited_outcomes", "contextual_states"):
        normalized.setdefault(field, [])
        if not isinstance(normalized[field], list):
            raise ArgumentError(f"provider learning state {field} must be a list")
    if len(normalized["arms"]) > 4096 or len(normalized["credited_outcomes"]) > 4096 or len(normalized["contextual_states"]) > 64:
        raise ArgumentError("provider learning state exceeds its bounded collection size")
    normalized_arms: list[dict[str, Any]] = []
    for item in normalized["arms"]:
        if not isinstance(item, Mapping):
            raise ArgumentError("provider learning state arms must contain mappings")
        normalized_arms.append(_arm(item.get("arm_id"), item))
    if len({item["arm_id"] for item in normalized_arms}) != len(normalized_arms):
        raise ArgumentError("provider learning state contains duplicate arms")
    normalized["arms"] = normalized_arms
    seen_outcomes: set[str] = set()
    credits: list[dict[str, Any]] = []
    for item in normalized["credited_outcomes"]:
        if not isinstance(item, Mapping):
            raise ArgumentError("provider learning state credited_outcomes must contain mappings")
        digest = _digest("provider learning state outcome_digest", item.get("outcome_digest"))
        if digest in seen_outcomes:
            raise ArgumentError("provider learning state contains duplicate outcome digests")
        seen_outcomes.add(digest)
        reward = item.get("reward")
        if isinstance(reward, bool) or not isinstance(reward, (int, float)) or not math.isfinite(float(reward)) or not -1 <= float(reward) <= 1:
            raise ArgumentError("provider learning state credited reward is malformed")
        if not isinstance(item.get("failed"), bool):
            raise ArgumentError("provider learning state credited failed is malformed")
        credits.append({
            "outcome_digest": digest,
            "arm_id": _text("provider learning state credited arm_id", item.get("arm_id"), 512),
            "reward": float(reward),
            "failed": item["failed"],
            "contract_digest": _digest("provider learning state contract_digest", item.get("contract_digest"), optional=True),
            "context_digest": _digest("provider learning state context_digest", item.get("context_digest"), optional=True),
        })
    normalized["credited_outcomes"] = credits
    contextual: list[dict[str, Any]] = []
    seen_contexts: set[str] = set()
    for item in normalized["contextual_states"]:
        if not isinstance(item, Mapping):
            raise ArgumentError("provider learning state contextual states must contain mappings")
        context_digest = _digest("provider learning state contextual context_digest", item.get("context_digest"))
        if context_digest in seen_contexts:
            raise ArgumentError("provider learning state contains duplicate contextual states")
        seen_contexts.add(context_digest)
        if not isinstance(item.get("context"), Mapping):
            raise ArgumentError("provider learning state contextual context is malformed")
        _, stable_context, expected = _normalize_context(item["context"])
        if expected != context_digest:
            raise ArgumentError("provider learning state contextual digest does not match its context")
        row_generation = item.get("generation", 0)
        if isinstance(row_generation, bool) or not isinstance(row_generation, int) or row_generation < 0:
            raise ArgumentError("provider learning state contextual generation is malformed")
        if not isinstance(item.get("observed", False), bool):
            raise ArgumentError("provider learning state contextual observed flag is malformed")
        raw_arms = item.get("arms", [])
        if not isinstance(raw_arms, list) or len(raw_arms) > 4096:
            raise ArgumentError("provider learning state contextual arms are malformed")
        contextual.append({
            "context_digest": context_digest,
            "context": stable_context,
            "generation": row_generation,
            "arms": [],
            "observed": item.get("observed", False),
        })
        contextual[-1]["arms"] = []
        for arm in raw_arms:
            if not isinstance(arm, Mapping):
                raise ArgumentError("provider learning state contextual arms must contain mappings")
            contextual[-1]["arms"].append(_arm(arm.get("arm_id"), arm))
        if len({arm["arm_id"] for arm in contextual[-1]["arms"]}) != len(contextual[-1]["arms"]):
            raise ArgumentError("provider learning state contextual states contain duplicate arms")
    normalized["contextual_states"] = contextual
    return normalized


def _arm(arm_id: str, raw: Mapping[str, Any] | None) -> dict[str, Any]:
    arm_id = _text("provider learning arm_id", arm_id, 512)
    current = {} if raw is None else dict(raw)
    if set(current).difference({"arm_id", "pulls", "reward_sum", "failures", "latency_ms", "disabled"}):
        raise ArgumentError("provider learning state arm contains unsupported fields")
    if current.get("arm_id", arm_id) != arm_id:
        raise ArgumentError("provider learning state arm identity is malformed")
    pulls = current.get("pulls", 0)
    failures = current.get("failures", 0)
    reward_sum = current.get("reward_sum", 0.0)
    if isinstance(pulls, bool) or not isinstance(pulls, int) or not 0 <= pulls <= 1_000_000_000:
        raise ArgumentError("provider learning state arm pulls are malformed")
    if isinstance(failures, bool) or not isinstance(failures, int) or not 0 <= failures <= pulls:
        raise ArgumentError("provider learning state arm failures are malformed")
    if isinstance(reward_sum, bool) or not isinstance(reward_sum, (int, float)) or not math.isfinite(float(reward_sum)) or not -pulls <= float(reward_sum) <= pulls:
        raise ArgumentError("provider learning state arm reward_sum is malformed")
    latency = current.get("latency_ms")
    if latency is not None:
        latency = _number("provider learning state arm latency_ms", latency, 86_400_000)
    disabled = current.get("disabled", False)
    if not isinstance(disabled, bool):
        raise ArgumentError("provider learning state arm disabled is malformed")
    return {"arm_id": arm_id, "pulls": pulls, "reward_sum": float(reward_sum), "failures": failures, "latency_ms": latency, "disabled": disabled}


def settle_autonomous_provider_model_outcome(
    state: Mapping[str, Any] | None,
    decision: Mapping[str, Any] | None = None,
    outcome: Mapping[str, Any] | None = None,
    *,
    arm_id: str | None = None,
    reward: float | None = None,
    failed: bool | None = None,
    outcome_digest: str | None = None,
    context_digest: str | None = None,
    context: Mapping[str, Any] | None = None,
    latency_ms: float | None = None,
) -> dict[str, Any]:
    """Apply one evaluator-approved model outcome to the portable bandit state.

    The positional ``decision``/``outcome`` form is accepted so this function can be passed
    directly as ``learning_updater``.  The keyword form is convenient for applications that
    settle a single receipt themselves.
    """

    decision = {} if decision is None else decision
    outcome = {} if outcome is None else outcome
    arm_id = arm_id or outcome.get("arm_id")
    reward = decision.get("reward") if reward is None else reward
    failed = decision.get("failed", False) if failed is None else failed
    outcome_digest = outcome.get("outcome_digest") if outcome_digest is None else outcome_digest
    context_digest = outcome.get("context_digest") if context_digest is None else context_digest
    context = outcome.get("context") if context is None else context
    latency_ms = outcome.get("latency_ms") if latency_ms is None else latency_ms
    # Model arms intentionally use the portable ``provider/model`` form; ``/`` is
    # therefore valid here even though receipt component identifiers are stricter.
    arm_id = _text("provider learning arm_id", arm_id, 512)
    if isinstance(reward, bool) or not isinstance(reward, (int, float)) or not math.isfinite(float(reward)) or not -1 <= float(reward) <= 1:
        raise ArgumentError("provider learning reward must be finite and within [-1, 1]")
    if not isinstance(failed, bool):
        raise ArgumentError("provider learning failed must be boolean")
    _digest("provider learning outcome_digest", outcome_digest)
    normalized_context: dict[str, Any] | None = None
    if context_digest is not None:
        if not isinstance(context, Mapping):
            raise ArgumentError("provider contextual learning requires context")
        normalized_context = _normalize_context(context)[1]
        if _ordered_digest(normalized_context) != context_digest:
            raise ArgumentError("provider learning context_digest does not match context")
    elif context is not None:
        raise ArgumentError("provider learning context requires context_digest")
    if latency_ms is not None:
        latency_ms = _number("provider learning latency_ms", latency_ms, 86_400_000)
    current = _state(state)
    prior = _prior_credit(current, outcome_digest)
    if prior is not None:
        if prior.get("arm_id") != arm_id or float(prior.get("reward")) != float(reward) or bool(prior.get("failed")) != failed or prior.get("context_digest") != context_digest:
            raise ArgumentError("provider learning outcome digest was reused with contradictory metadata")
        return current
    target_container = current["arms"]
    contextual_row: dict[str, Any] | None = None
    if context_digest is not None:
        contextual_row = next((row for row in current["contextual_states"] if isinstance(row, Mapping) and row.get("context_digest") == context_digest), None)
        if contextual_row is None:
            contextual_row = {"context_digest": context_digest, "context": normalized_context, "generation": 0, "arms": [], "observed": False}
            current["contextual_states"].append(contextual_row)
        target_container = contextual_row["arms"]
    prior_arm = next((item for item in target_container if isinstance(item, Mapping) and item.get("arm_id") == arm_id), None)
    current_arm = _arm(arm_id, prior_arm)
    pulls = current_arm["pulls"]
    current_arm["pulls"] = pulls + 1
    current_arm["reward_sum"] = round(current_arm["reward_sum"] + float(reward), 12)
    current_arm["failures"] = current_arm["failures"] + int(failed)
    if latency_ms is not None:
        previous_latency = current_arm["latency_ms"]
        current_arm["latency_ms"] = round(((float(previous_latency) * pulls if previous_latency is not None else 0.0) + float(latency_ms)) / (pulls + 1), 6)
    target_container[:] = [item for item in target_container if item.get("arm_id") != arm_id] + [current_arm]
    if len(target_container) > 4096:
        raise ArgumentError("provider learning state has reached its arm bound")
    current["credited_outcomes"].append({"outcome_digest": outcome_digest, "arm_id": arm_id, "reward": float(reward), "failed": failed, "contract_digest": outcome.get("contract_digest"), "context_digest": context_digest})
    current["generation"] += 1
    if contextual_row is not None:
        contextual_row["generation"] = int(contextual_row.get("generation", 0)) + 1
        contextual_row["observed"] = True
    current["arms"] = sorted(current["arms"], key=lambda item: item["arm_id"])
    current["credited_outcomes"] = sorted(current["credited_outcomes"], key=lambda item: item["outcome_digest"])
    current["contextual_states"] = sorted(current["contextual_states"], key=lambda item: item["context_digest"])
    return _safe_json("next provider learning state", current)


__all__ = [
    "AUTONOMOUS_PROVIDER_EVALUATION_SCHEMA",
    "AUTONOMOUS_PROVIDER_LEARNING_SCHEMA",
    "MAX_AUTONOMOUS_PROVIDER_EVALUATION_EVIDENCE_BYTES",
    "MAX_AUTONOMOUS_PROVIDER_EVALUATION_RECEIPTS",
    "AutonomousProviderOutcomeContext",
    "AutonomousProviderOutcomeEvaluationInput",
    "AutonomousProviderEvaluatorAssessment",
    "AutonomousProviderEvaluation",
    "AutonomousProviderOutcomeEvaluator",
    "AutonomousProviderLearningReport",
    "autonomous_provider_receipt_identity",
    "autonomous_provider_outcome_evaluation_input",
    "settle_autonomous_provider_model_outcome",
]
