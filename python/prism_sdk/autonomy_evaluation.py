"""Evaluator, bandit, and replay boundaries for autonomous domain-tool outcomes.

Tool execution deliberately returns only a bounded metadata receipt.  This module turns that
receipt into an evaluator input, validates a compact value-only judgment, optionally applies a
caller-owned bandit update, and records replay metadata.  It never hands tool arguments, tool
outputs, prompts, provider messages, or credentials to the learning ledger.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
import math
from typing import Any, Callable, Mapping, Sequence

from .authoring import canonical_bytes, content_digest
from .brain import BrainLearningLedger
from .domain_tools import (
    AutonomousDomainToolReceipt,
    DOMAIN_TOOL_EXECUTION_STATUSES,
    MAX_DOMAIN_TOOL_CALLS,
)
from .errors import ArgumentError
from .autonomy_persistence import (
    AutonomousExecutionController,
    AutonomyPersistenceError,
)


AUTONOMOUS_TOOL_EVALUATION_SCHEMA = "bioprism-python-autonomous-tool-evaluation/0.1"
AUTONOMOUS_TOOL_REPLAY_CASE_SCHEMA = "bioprism-python-autonomous-tool-replay-case/0.1"
AUTONOMOUS_TOOL_REPLAY_REPORT_SCHEMA = "bioprism-python-autonomous-tool-replay-report/0.1"
AUTONOMOUS_TOOL_LEARNING_SCHEMA = "bioprism-python-autonomous-tool-learning/0.1"
MAX_TOOL_EVALUATION_EVIDENCE_BYTES = 256_000
MAX_TOOL_REPLAY_CASES = 4_096
MAX_TOOL_REPLAY_EVIDENCE_BYTES = 256_000
_SAFE_IDENTIFIER_CHARS = frozenset("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.-")
_FORBIDDEN_FIELDS = frozenset(
    {
        "apikey", "authorization", "bearer", "credential", "password", "secret",
        "accesstoken", "refreshtoken", "token", "privatekey", "prompt", "response",
        "rawpayload", "arguments", "output", "task", "messages",
    }
)


def _identifier(name: str, value: Any, *, maximum: int = 512) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise ArgumentError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > maximum or any(character not in _SAFE_IDENTIFIER_CHARS for character in value):
        raise ArgumentError(f"{name} must be a bounded identifier")
    return value


def _digest(name: str, value: Any) -> str:
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _assert_safe(value: Any, *, depth: int = 0) -> None:
    if depth > 32:
        raise ArgumentError("tool evaluator evidence is too deeply nested")
    if isinstance(value, Mapping):
        for key, child in value.items():
            normalized = "".join(character for character in key.lower() if character.isalnum()) if isinstance(key, str) else ""
            if normalized in _FORBIDDEN_FIELDS:
                raise ArgumentError("tool evaluator evidence contains transient or secret-shaped fields")
            _assert_safe(child, depth=depth + 1)
    elif isinstance(value, (list, tuple)):
        for child in value:
            _assert_safe(child, depth=depth + 1)
    elif isinstance(value, float) and not math.isfinite(value):
        raise ArgumentError("tool evaluator evidence contains a non-finite number")


def _safe_json(name: str, value: Any, *, maximum: int) -> Any:
    _assert_safe(value)
    try:
        encoded = canonical_bytes(value)
    except (TypeError, ValueError, ArgumentError) as error:
        raise ArgumentError(f"{name} must be canonical JSON") from error
    if len(encoded) > maximum:
        raise ArgumentError(f"{name} exceeds its bounded size")
    return json.loads(encoded.decode("utf-8"))


def _receipt_identity(receipt: AutonomousDomainToolReceipt) -> str:
    """Return the stable batch identity without changing the public receipt schema."""

    execution_id = receipt.execution_id or "unjournaled"
    return f"{execution_id}:{receipt.call_id}"


@dataclass(frozen=True, slots=True)
class AutonomousToolOutcomeEvidence:
    """Safe evaluator input projection for one tool outcome."""

    execution_id: str
    domain: str
    capability: str
    risk_class: str
    call_id: str
    tool: str
    status: str
    schema_digest: str | None = None
    arguments_digest: str | None = None
    output_digest: str | None = None
    evidence: Mapping[str, Any] | None = None

    def __post_init__(self) -> None:
        for name, value in (
            ("execution_id", self.execution_id), ("domain", self.domain), ("capability", self.capability),
            ("risk_class", self.risk_class), ("call_id", self.call_id), ("tool", self.tool),
        ):
            _identifier(f"tool evidence {name}", value)
        if self.status not in DOMAIN_TOOL_EXECUTION_STATUSES:
            raise ArgumentError("tool evidence status is invalid")
        for name, value in (("schema_digest", self.schema_digest), ("arguments_digest", self.arguments_digest), ("output_digest", self.output_digest)):
            if value is not None:
                _digest(f"tool evidence {name}", value)
        if self.evidence is not None:
            if not isinstance(self.evidence, Mapping):
                raise ArgumentError("tool evaluator evidence must be a mapping or None")
            object.__setattr__(self, "evidence", _safe_json("tool evaluator evidence", dict(self.evidence), maximum=MAX_TOOL_EVALUATION_EVIDENCE_BYTES))

    @classmethod
    def from_receipt(
        cls,
        receipt: AutonomousDomainToolReceipt,
        *,
        execution_id: str | None = None,
        domain: str | None = None,
        capability: str | None = None,
        risk_class: str | None = None,
        evidence: Mapping[str, Any] | None = None,
    ) -> "AutonomousToolOutcomeEvidence":
        if not isinstance(receipt, AutonomousDomainToolReceipt):
            raise ArgumentError("tool outcome evidence requires an AutonomousDomainToolReceipt")
        return cls(
            execution_id=execution_id or receipt.execution_id or "unjournaled",
            domain=domain or receipt.domain or "cross_domain",
            capability=capability or receipt.capability or "tool_execution",
            risk_class=risk_class or receipt.risk_class or "read_only",
            call_id=receipt.call_id,
            tool=receipt.tool,
            status=receipt.status,
            schema_digest=receipt.schema_digest,
            arguments_digest=receipt.arguments_digest,
            output_digest=receipt.output_digest,
            evidence=evidence,
        )

    @property
    def evidence_digest(self) -> str:
        return content_digest(self.to_input()["evidence"])

    def to_input(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_TOOL_EVALUATION_SCHEMA,
            "execution_id": self.execution_id,
            "domain": self.domain,
            "capability": self.capability,
            "risk_class": self.risk_class,
            "call_id": self.call_id,
            "tool": self.tool,
            "status": self.status,
            "schema_digest": self.schema_digest,
            "arguments_digest": self.arguments_digest,
            "output_digest": self.output_digest,
            "evidence": {} if self.evidence is None else dict(self.evidence),
            "retention": "digests_and_safe_evidence_only_no_arguments_or_outputs",
        }

    def to_dict(self) -> dict[str, Any]:
        result = self.to_input()
        result["evidence_digest"] = self.evidence_digest
        result["evidence"] = None
        return result


@dataclass(frozen=True, slots=True)
class AutonomousToolEvaluation:
    """Validated compact evaluator judgment."""

    evaluator_id: str
    evaluator_version: str
    reward: float
    passed: bool
    failed: bool = False
    feedback_digest: str | None = None
    failure_class: str | None = None
    evidence_digest: str | None = None
    decision_digest: str | None = None

    def __post_init__(self) -> None:
        _identifier("tool evaluator_id", self.evaluator_id)
        _identifier("tool evaluator_version", self.evaluator_version)
        if not isinstance(self.reward, (int, float)) or isinstance(self.reward, bool) or not math.isfinite(float(self.reward)) or not -1 <= float(self.reward) <= 1:
            raise ArgumentError("tool evaluator reward must be finite and within [-1, 1]")
        if not isinstance(self.passed, bool) or not isinstance(self.failed, bool):
            raise ArgumentError("tool evaluator passed and failed must be booleans")
        if self.passed and self.failed:
            raise ArgumentError("tool evaluator cannot be both passed and failed")
        for name, value in (("feedback_digest", self.feedback_digest), ("evidence_digest", self.evidence_digest), ("decision_digest", self.decision_digest)):
            if value is not None:
                _digest(f"tool evaluator {name}", value)
        if self.failure_class is not None:
            _identifier("tool evaluator failure_class", self.failure_class)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_TOOL_EVALUATION_SCHEMA,
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "reward": float(self.reward),
            "passed": self.passed,
            "failed": self.failed,
            "feedback_digest": self.feedback_digest,
            "failure_class": self.failure_class,
            "evidence_digest": self.evidence_digest,
            "decision_digest": self.decision_digest,
            "retention": "value_only",
        }


class AutonomousToolOutcomeEvaluator:
    """Adapt a caller-owned evaluator to the domain-tool learning boundary."""

    _ALLOWED_DECISION_FIELDS = frozenset({"reward", "passed", "failed", "feedback_digest", "failure_class"})

    def __init__(
        self,
        evaluator: Callable[[Mapping[str, Any]], Mapping[str, Any] | AutonomousToolEvaluation],
        *,
        evaluator_id: str,
        evaluator_version: str,
    ) -> None:
        if not callable(evaluator):
            raise ArgumentError("tool evaluator must be callable")
        _identifier("tool evaluator_id", evaluator_id)
        _identifier("tool evaluator_version", evaluator_version)
        self.evaluator = evaluator
        self.evaluator_id = evaluator_id
        self.evaluator_version = evaluator_version

    def assess(self, outcome: AutonomousToolOutcomeEvidence) -> AutonomousToolEvaluation:
        if not isinstance(outcome, AutonomousToolOutcomeEvidence):
            raise ArgumentError("tool evaluator requires AutonomousToolOutcomeEvidence")
        try:
            raw = self.evaluator(outcome.to_input())
        except Exception as error:
            raise ArgumentError("tool evaluator callback failed") from error
        if isinstance(raw, AutonomousToolEvaluation):
            if raw.evaluator_id != self.evaluator_id or raw.evaluator_version != self.evaluator_version:
                raise ArgumentError("tool evaluator decision identity does not match the evaluator")
            decision = raw
        else:
            if not isinstance(raw, Mapping):
                raise ArgumentError("tool evaluator callback must return a mapping")
            _assert_safe(raw)
            if set(raw).difference(self._ALLOWED_DECISION_FIELDS) or "reward" not in raw or "passed" not in raw:
                raise ArgumentError("tool evaluator decision has unsupported or missing fields")
            passed = raw["passed"]
            failed = raw.get("failed", not passed)
            if not isinstance(passed, bool) or not isinstance(failed, bool):
                raise ArgumentError("tool evaluator passed and failed must be booleans")
            decision = AutonomousToolEvaluation(
                evaluator_id=self.evaluator_id,
                evaluator_version=self.evaluator_version,
                reward=raw["reward"],
                passed=passed,
                failed=failed,
                feedback_digest=raw.get("feedback_digest"),
                failure_class=raw.get("failure_class"),
                evidence_digest=outcome.evidence_digest,
            )
        if decision.evidence_digest not in (None, outcome.evidence_digest):
            raise ArgumentError("tool evaluator evidence_digest does not match the input")
        if decision.evidence_digest is None:
            decision = AutonomousToolEvaluation(
                evaluator_id=decision.evaluator_id,
                evaluator_version=decision.evaluator_version,
                reward=decision.reward,
                passed=decision.passed,
                failed=decision.failed,
                feedback_digest=decision.feedback_digest,
                failure_class=decision.failure_class,
                evidence_digest=outcome.evidence_digest,
            )
        digest = content_digest(decision.to_dict())
        return AutonomousToolEvaluation(
            evaluator_id=decision.evaluator_id,
            evaluator_version=decision.evaluator_version,
            reward=decision.reward,
            passed=decision.passed,
            failed=decision.failed,
            feedback_digest=decision.feedback_digest,
            failure_class=decision.failure_class,
            evidence_digest=decision.evidence_digest,
            decision_digest=digest,
        )

    def evaluate_and_record(
        self,
        outcome: AutonomousToolOutcomeEvidence,
        *,
        controller: AutonomousExecutionController | None = None,
        bandit_state: Mapping[str, Any] | None = None,
        bandit_updater: Callable[[Mapping[str, Any], Mapping[str, Any], Mapping[str, Any]], Mapping[str, Any]] | None = None,
        ledger: BrainLearningLedger | None = None,
    ) -> dict[str, Any]:
        decision = self.assess(outcome)
        if controller is not None:
            if controller.state.execution_id != outcome.execution_id:
                raise ArgumentError("tool outcome execution_id does not match the controller")
            try:
                controller.record_evaluation(
                    evaluator_id=decision.evaluator_id,
                    evaluator_version=decision.evaluator_version,
                    reward=decision.reward,
                    passed=decision.passed,
                    evaluation_digest=decision.decision_digest or content_digest(decision.to_dict()),
                    failure_class=decision.failure_class,
                )
            except AutonomyPersistenceError as error:
                raise ArgumentError("tool evaluation could not be journaled") from error
        if bandit_state is not None:
            if not isinstance(bandit_state, Mapping):
                raise ArgumentError("bandit_state must be a mapping or None")
            _safe_json("bandit_state", dict(bandit_state), maximum=MAX_TOOL_EVALUATION_EVIDENCE_BYTES)
        next_state: Mapping[str, Any] = {} if bandit_state is None else dict(bandit_state)
        if bandit_updater is not None:
            if not callable(bandit_updater):
                raise ArgumentError("bandit_updater must be callable")
            try:
                next_state = bandit_updater(dict(next_state), decision.to_dict(), outcome.to_dict())
            except Exception as error:
                raise ArgumentError("bandit updater failed") from error
            if not isinstance(next_state, Mapping):
                raise ArgumentError("bandit updater must return a mapping")
            next_state = _safe_json("next bandit state", dict(next_state), maximum=MAX_TOOL_EVALUATION_EVIDENCE_BYTES)
        learning_evidence = {
            "schema": AUTONOMOUS_TOOL_EVALUATION_SCHEMA,
            "execution_id": outcome.execution_id,
            "domain": outcome.domain,
            "capability": outcome.capability,
            "risk_class": outcome.risk_class,
            "call_id": outcome.call_id,
            "tool": outcome.tool,
            "status": outcome.status,
            "outcome_digest": outcome.output_digest,
            "arguments_digest": outcome.arguments_digest,
            "evidence_digest": outcome.evidence_digest,
            "decision_digest": decision.decision_digest,
            "evaluator_id": decision.evaluator_id,
            "evaluator_version": decision.evaluator_version,
            "reward": decision.reward,
            "passed": decision.passed,
            "failed": decision.failed,
            "failure_class": decision.failure_class,
            "retention": "metadata_only_no_arguments_or_outputs",
        }
        report = {"learning_evidence": learning_evidence, "next_state": dict(next_state)}
        replay = {
            "schema": AUTONOMOUS_TOOL_REPLAY_CASE_SCHEMA,
            "execution_id": outcome.execution_id,
            "domain": outcome.domain,
            "tool": outcome.tool,
            "call_id": outcome.call_id,
            "evidence_digest": outcome.evidence_digest,
            "decision_digest": decision.decision_digest,
            "evaluator_id": decision.evaluator_id,
            "evaluator_version": decision.evaluator_version,
            "retention": "metadata_and_digests_only",
        }
        recording = None
        if ledger is not None:
            if not isinstance(ledger, BrainLearningLedger):
                raise ArgumentError("ledger must be a BrainLearningLedger or None")
            recording = ledger.append(
                report,
                context_digest=content_digest({"domain": outcome.domain, "capability": outcome.capability, "risk_class": outcome.risk_class}),
                replay=replay,
            )
        return {
            "schema": AUTONOMOUS_TOOL_EVALUATION_SCHEMA,
            "decision": decision.to_dict(),
            "learning_evidence": learning_evidence,
            "next_state": dict(next_state),
            "recording": recording,
            "replay": replay,
            "retention": "metadata_only",
        }

    def evaluate_receipts(
        self,
        receipts: Sequence[AutonomousDomainToolReceipt],
        *,
        evidence: Mapping[str, Mapping[str, Any]] | None = None,
        controller: AutonomousExecutionController | None = None,
        bandit_state: Mapping[str, Any] | None = None,
        bandit_updater: Callable[[Mapping[str, Any], Mapping[str, Any], Mapping[str, Any]], Mapping[str, Any]] | None = None,
        ledger: BrainLearningLedger | None = None,
    ) -> "AutonomousToolLearningReport":
        """Evaluate a bounded live receipt batch and advance caller-owned online state.

        Transport status is intentionally not a reward. The caller supplies optional, safe
        per-call evidence (keyed by ``call_id``) when those IDs are unique, or by the namespaced
        ``execution_id:call_id`` identity when a provider reuses an ID in another execution. The
        evaluator receives only the receipt projection plus that evidence. Evaluations are
        applied in receipt order, so a bandit updater sees a deterministic stream and the
        returned state is immediately usable by the next autonomous run.
        """

        if (
            not isinstance(receipts, Sequence)
            or isinstance(receipts, (str, bytes))
            or len(receipts) > MAX_DOMAIN_TOOL_CALLS
        ):
            raise ArgumentError(f"tool receipt batches must contain at most {MAX_DOMAIN_TOOL_CALLS} entries")
        if any(not isinstance(receipt, AutonomousDomainToolReceipt) for receipt in receipts):
            raise ArgumentError("tool receipt batches must contain AutonomousDomainToolReceipt values")
        call_ids = [receipt.call_id for receipt in receipts]
        identities = [_receipt_identity(receipt) for receipt in receipts]
        if len(set(identities)) != len(identities):
            raise ArgumentError("tool receipt batches cannot contain duplicate execution_id/call_id identities")
        if evidence is not None and not isinstance(evidence, Mapping):
            raise ArgumentError("tool receipt evidence must be a mapping or None")
        evidence_by_call = {} if evidence is None else dict(evidence)
        if any(not isinstance(call_id, str) for call_id in evidence_by_call):
            raise ArgumentError("tool receipt evidence keys must be call_id or execution_id:call_id strings")
        unique_call_ids = {call_id for call_id in call_ids if call_ids.count(call_id) == 1}
        valid_evidence_keys = set(identities).union(unique_call_ids)
        unknown_evidence = sorted(set(evidence_by_call).difference(valid_evidence_keys))
        if unknown_evidence:
            raise ArgumentError("tool receipt evidence contains an unknown receipt identity")
        for call_id, packet in evidence_by_call.items():
            if not isinstance(call_id, str) or not isinstance(packet, Mapping):
                raise ArgumentError("tool receipt evidence must map receipt identity strings to mappings")
            _safe_json(f"tool receipt evidence for {call_id}", dict(packet), maximum=MAX_TOOL_EVALUATION_EVIDENCE_BYTES)
        if bandit_state is not None and not isinstance(bandit_state, Mapping):
            raise ArgumentError("bandit_state must be a mapping or None")
        state: Mapping[str, Any] = {} if bandit_state is None else _safe_json(
            "bandit_state", dict(bandit_state), maximum=MAX_TOOL_EVALUATION_EVIDENCE_BYTES
        )
        evaluations: list[Mapping[str, Any]] = []
        by_domain: dict[str, int] = {}
        by_status: dict[str, int] = {}
        for receipt in receipts:
            receipt_evidence = evidence_by_call.get(_receipt_identity(receipt))
            if receipt_evidence is None and receipt.call_id in unique_call_ids:
                receipt_evidence = evidence_by_call.get(receipt.call_id)
            outcome = AutonomousToolOutcomeEvidence.from_receipt(
                receipt,
                evidence=receipt_evidence,
            )
            report = self.evaluate_and_record(
                outcome,
                controller=controller,
                bandit_state=state,
                bandit_updater=bandit_updater,
                ledger=ledger,
            )
            decision = report["decision"]
            evaluations.append(
                {
                    "execution_id": outcome.execution_id,
                    "domain": outcome.domain,
                    "capability": outcome.capability,
                    "risk_class": outcome.risk_class,
                    "call_id": outcome.call_id,
                    "tool": outcome.tool,
                    "status": outcome.status,
                    "evidence_digest": outcome.evidence_digest,
                    "decision_digest": decision.get("decision_digest"),
                    "evaluator_id": decision.get("evaluator_id"),
                    "evaluator_version": decision.get("evaluator_version"),
                    "reward": decision.get("reward"),
                    "passed": decision.get("passed"),
                    "failed": decision.get("failed"),
                    "failure_class": decision.get("failure_class"),
                    "recording": report.get("recording"),
                }
            )
            next_state = report.get("next_state")
            if not isinstance(next_state, Mapping):
                raise ArgumentError("tool evaluator returned a malformed next bandit state")
            state = dict(next_state)
            by_domain[outcome.domain] = by_domain.get(outcome.domain, 0) + 1
            by_status[outcome.status] = by_status.get(outcome.status, 0) + 1
        return AutonomousToolLearningReport(
            status="completed" if receipts else "no_receipts",
            receipts=len(receipts),
            evaluations=tuple(evaluations),
            by_domain=dict(sorted(by_domain.items())),
            by_status=dict(sorted(by_status.items())),
            next_bandit_state=dict(state),
            learning_digest=content_digest(evaluations),
        )


@dataclass(frozen=True, slots=True)
class AutonomousToolLearningReport:
    """Metadata-only result of live domain-tool evaluator and bandit settlement."""

    status: str
    receipts: int
    evaluations: tuple[Mapping[str, Any], ...]
    by_domain: Mapping[str, int]
    by_status: Mapping[str, int]
    next_bandit_state: Mapping[str, Any]
    learning_digest: str

    def __post_init__(self) -> None:
        if self.status not in {"completed", "no_receipts"}:
            raise ArgumentError("tool learning report status is invalid")
        if not isinstance(self.receipts, int) or isinstance(self.receipts, bool) or self.receipts < 0:
            raise ArgumentError("tool learning report receipts must be a non-negative integer")
        if self.receipts != len(self.evaluations):
            raise ArgumentError("tool learning report receipt count does not match evaluations")
        if self.status == "no_receipts" and self.receipts != 0:
            raise ArgumentError("empty tool learning reports must have no receipts")
        _digest("tool learning digest", self.learning_digest)
        _safe_json("tool learning report evaluations", [dict(item) for item in self.evaluations], maximum=MAX_TOOL_EVALUATION_EVIDENCE_BYTES)
        _safe_json("tool learning report bandit state", dict(self.next_bandit_state), maximum=MAX_TOOL_EVALUATION_EVIDENCE_BYTES)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_TOOL_LEARNING_SCHEMA,
            "status": self.status,
            "receipts": self.receipts,
            "evaluations": [dict(item) for item in self.evaluations],
            "by_domain": dict(self.by_domain),
            "by_status": dict(self.by_status),
            "next_bandit_state": dict(self.next_bandit_state),
            "learning_digest": self.learning_digest,
            "retention": "metadata_and_digests_only",
        }


@dataclass(frozen=True, slots=True)
class AutonomousToolReplayCase:
    """A caller-rehydrated safe case for deterministic evaluator replay."""

    execution_id: str
    domain: str
    capability: str
    risk_class: str
    call_id: str
    tool: str
    status: str
    schema_digest: str | None = None
    arguments_digest: str | None = None
    output_digest: str | None = None
    evidence: Mapping[str, Any] | None = None
    expected_decision_digest: str | None = None

    def __post_init__(self) -> None:
        outcome = AutonomousToolOutcomeEvidence(
            execution_id=self.execution_id, domain=self.domain, capability=self.capability,
            risk_class=self.risk_class, call_id=self.call_id, tool=self.tool, status=self.status,
            schema_digest=self.schema_digest, arguments_digest=self.arguments_digest,
            output_digest=self.output_digest, evidence=self.evidence,
        )
        object.__setattr__(self, "evidence", outcome.evidence)
        if self.expected_decision_digest is not None:
            _digest("expected_decision_digest", self.expected_decision_digest)

    def outcome(self) -> AutonomousToolOutcomeEvidence:
        return AutonomousToolOutcomeEvidence(
            execution_id=self.execution_id, domain=self.domain, capability=self.capability,
            risk_class=self.risk_class, call_id=self.call_id, tool=self.tool, status=self.status,
            schema_digest=self.schema_digest, arguments_digest=self.arguments_digest,
            output_digest=self.output_digest, evidence=self.evidence,
        )

    def to_dict(self) -> dict[str, Any]:
        outcome = self.outcome()
        return {
            "schema": AUTONOMOUS_TOOL_REPLAY_CASE_SCHEMA,
            **outcome.to_dict(),
            "expected_decision_digest": self.expected_decision_digest,
            "retention": "safe_evidence_and_digests_no_arguments_or_outputs",
        }


@dataclass(frozen=True, slots=True)
class AutonomousToolReplayReport:
    """Metadata-only result of replaying tool evaluator cases."""

    cases: int
    decisions: tuple[Mapping[str, Any], ...]
    disagreements: int
    by_domain: Mapping[str, int]
    next_bandit_state: Mapping[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_TOOL_REPLAY_REPORT_SCHEMA,
            "cases": self.cases,
            "decisions": [dict(item) for item in self.decisions],
            "disagreements": self.disagreements,
            "by_domain": dict(self.by_domain),
            "next_bandit_state": dict(self.next_bandit_state),
            "retention": "metadata_and_digests_only",
        }


class AutonomousToolReplayEngine:
    """Replay value-only cases without invoking a provider or a domain tool."""

    def replay(
        self,
        cases: Sequence[AutonomousToolReplayCase],
        evaluator: AutonomousToolOutcomeEvaluator,
        *,
        bandit_state: Mapping[str, Any] | None = None,
        bandit_updater: Callable[[Mapping[str, Any], Mapping[str, Any], Mapping[str, Any]], Mapping[str, Any]] | None = None,
    ) -> AutonomousToolReplayReport:
        if not isinstance(cases, Sequence) or isinstance(cases, (str, bytes)) or len(cases) > MAX_TOOL_REPLAY_CASES:
            raise ArgumentError(f"replay cases must contain at most {MAX_TOOL_REPLAY_CASES} entries")
        if any(not isinstance(case, AutonomousToolReplayCase) for case in cases):
            raise ArgumentError("replay cases must contain AutonomousToolReplayCase values")
        if not isinstance(evaluator, AutonomousToolOutcomeEvaluator):
            raise ArgumentError("replay requires an AutonomousToolOutcomeEvaluator")
        state: Mapping[str, Any] = {} if bandit_state is None else _safe_json("bandit_state", dict(bandit_state), maximum=MAX_TOOL_EVALUATION_EVIDENCE_BYTES)
        decisions: list[Mapping[str, Any]] = []
        by_domain: dict[str, int] = {}
        disagreements = 0
        for case in cases:
            outcome = case.outcome()
            decision = evaluator.assess(outcome)
            disagreement = case.expected_decision_digest is not None and case.expected_decision_digest != decision.decision_digest
            disagreements += int(disagreement)
            by_domain[case.domain] = by_domain.get(case.domain, 0) + 1
            decisions.append({
                "execution_id": case.execution_id,
                "domain": case.domain,
                "tool": case.tool,
                "call_id": case.call_id,
                "evidence_digest": outcome.evidence_digest,
                "decision_digest": decision.decision_digest,
                "expected_decision_digest": case.expected_decision_digest,
                "disagreement": disagreement,
                "evaluator_id": decision.evaluator_id,
                "evaluator_version": decision.evaluator_version,
                "reward": decision.reward,
                "passed": decision.passed,
                "failed": decision.failed,
            })
            if bandit_updater is not None:
                try:
                    updated = bandit_updater(dict(state), decision.to_dict(), outcome.to_dict())
                except Exception as error:
                    raise ArgumentError("bandit updater failed during replay") from error
                if not isinstance(updated, Mapping):
                    raise ArgumentError("bandit updater must return a mapping")
                state = _safe_json("replayed bandit state", dict(updated), maximum=MAX_TOOL_EVALUATION_EVIDENCE_BYTES)
        return AutonomousToolReplayReport(
            cases=len(cases),
            decisions=tuple(decisions),
            disagreements=disagreements,
            by_domain=dict(sorted(by_domain.items())),
            next_bandit_state=dict(state),
        )


__all__ = [
    "AUTONOMOUS_TOOL_EVALUATION_SCHEMA",
    "AUTONOMOUS_TOOL_LEARNING_SCHEMA",
    "AUTONOMOUS_TOOL_REPLAY_CASE_SCHEMA",
    "AUTONOMOUS_TOOL_REPLAY_REPORT_SCHEMA",
    "AutonomousToolOutcomeEvidence",
    "AutonomousToolEvaluation",
    "AutonomousToolOutcomeEvaluator",
    "AutonomousToolLearningReport",
    "AutonomousToolReplayCase",
    "AutonomousToolReplayEngine",
    "AutonomousToolReplayReport",
]
