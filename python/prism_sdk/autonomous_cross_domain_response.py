"""Digest-bound integrity and alignment gating for cross-domain responses.

Cross-domain execution already provides bounded specialist fan-out and a synthesis call.  That
execution envelope is intentionally agnostic about whether each specialist produced a useful,
complete, or mutually compatible structured response.  This module supplies the missing
provider-free gate.

Each specialist response is validated against its reviewed domain contract and evaluated by the
existing structural response evaluator.  An optional caller-owned alignment catalogue binds
pairwise support, contradiction, neutrality, or unresolved disagreement to the exact response
digests.  The resulting projection contains only digests, counts, scores, and bounded next
actions.  It never retains answer text, evidence text, prompts, credentials, or provider values.

The alignment records are deliberately explicit.  This module does not pretend that lexical
overlap or a structural score establishes scientific truth; a semantic evaluator or operator may
provide an alignment record, but that record remains a review signal and must not be confused with
external-world verification.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
import math
import re
from typing import Any, Mapping, Sequence

from .authoring import content_digest
from .autonomous_domain_response import (
    AutonomousDomainResponseContract,
    AutonomousDomainResponseEvaluation,
    evaluate_autonomous_domain_response,
    validate_autonomous_domain_response,
)
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError


AUTONOMOUS_CROSS_DOMAIN_RESPONSE_SCHEMA = "bioprism-python-autonomous-cross-domain-response/0.1"
AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ROW_SCHEMA = "bioprism-python-autonomous-cross-domain-response-row/0.1"
AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ALIGNMENT_SCHEMA = "bioprism-python-autonomous-cross-domain-response-alignment/0.1"
AUTONOMOUS_CROSS_DOMAIN_RESPONSE_STATUSES = (
    "ready_to_synthesize",
    "needs_alignment_review",
    "partial",
    "blocked",
    "completed",
)
AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ROLES = ("specialist", "synthesis")
AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ALIGNMENT_STANCES = (
    "support",
    "contradict",
    "neutral",
    "unresolved",
)
MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ENTRIES = len(AUTONOMOUS_DOMAIN_NAMES)
MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ALIGNMENTS = 128
MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ACTIONS = 32
MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_REASONS = 32
MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_BYTES = 512_000
AUTONOMOUS_CROSS_DOMAIN_RESPONSE_MIN_REWARD = 0.8
AUTONOMOUS_CROSS_DOMAIN_RESPONSE_MIN_ALIGNMENT_CONFIDENCE = 0.75
AUTONOMOUS_CROSS_DOMAIN_RESPONSE_CONTRADICTION_CONFIDENCE = 0.75

_DIGEST = re.compile(r"^[0-9a-f]{64}$")
_IDENTIFIER = re.compile(r"^[A-Za-z0-9_.:-]+$")
_RETENTION = "metadata_only;responses_prompts_credentials_and_provider_values_not_retained"
_AUTHORITY = "structural_and_caller_alignment_metadata_only;not_external_truth"
_ALIGNMENT_PAIR_SEPARATOR = "::"


def _field(value: Any, name: str, default: Any = None) -> Any:
    if isinstance(value, Mapping):
        return value.get(name, default)
    return getattr(value, name, default)


def _bounded_text(name: str, value: Any, maximum: int = 512) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise ArgumentError(f"{name} is outside its bounded text contract")
    if len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} exceeds its byte bound")
    return value.strip()


def _identifier(name: str, value: Any, maximum: int = 256) -> str:
    text = _bounded_text(name, value, maximum)
    if not _IDENTIFIER.fullmatch(text):
        raise ArgumentError(f"{name} is not a bounded identifier")
    return text


def _digest(name: str, value: Any) -> str:
    text = _bounded_text(name, value, 64)
    if not _DIGEST.fullmatch(text):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return text


def _optional_digest(name: str, value: Any) -> str | None:
    return None if value is None else _digest(name, value)


def _bounded_strings(name: str, value: Any, maximum: int = MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ACTIONS) -> tuple[str, ...]:
    if isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence) or len(value) > maximum:
        raise ArgumentError(f"{name} must be a bounded sequence")
    result = tuple(_bounded_text(f"{name} entry", item, 1_024) for item in value)
    if len(set(result)) != len(result):
        raise ArgumentError(f"{name} contains duplicate entries")
    return result


def _json_bytes(value: Any) -> int:
    try:
        return len(json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8"))
    except (TypeError, ValueError) as error:
        raise ArgumentError("cross-domain response assessment must be canonical JSON") from error


def _assert_safe_metadata(value: Any, name: str = "cross-domain response assessment", depth: int = 0) -> None:
    if depth > 16:
        raise ArgumentError(f"{name} is too deeply nested")
    if isinstance(value, Mapping):
        if len(value) > 512:
            raise ArgumentError(f"{name} contains too many fields")
        for key, child in value.items():
            if not isinstance(key, str) or not key.strip():
                raise ArgumentError(f"{name} contains an invalid field name")
            normalized = "".join(character for character in key.lower() if character.isalnum())
            if normalized == "secretmaterial" and child == "never_returned":
                _assert_safe_metadata(child, f"{name}.{key}", depth + 1)
                continue
            if normalized in {
                "apikey", "authorization", "bearer", "credential", "credentials", "password", "secret",
                "secretkey", "token", "accesstoken", "refreshtoken", "privatekey", "clientsecret",
            } or any(marker in normalized for marker in ("token", "secret", "credential")):
                raise ArgumentError(f"{name}.{key} is credential-shaped metadata")
            _assert_safe_metadata(child, f"{name}.{key}", depth + 1)
        return
    if isinstance(value, (list, tuple)):
        if len(value) > 512:
            raise ArgumentError(f"{name} contains too many entries")
        for index, child in enumerate(value):
            _assert_safe_metadata(child, f"{name}[{index}]", depth + 1)
        return
    if isinstance(value, float) and not math.isfinite(value):
        raise ArgumentError(f"{name} contains a non-finite number")


def _domains(name: str, value: Any, *, minimum: int = 2) -> tuple[str, ...]:
    if isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence):
        raise ArgumentError(f"{name} must be a domain sequence")
    if not minimum <= len(value) <= MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ENTRIES:
        raise ArgumentError(f"{name} is outside its domain bound")
    result = tuple(_bounded_text(f"{name} entry", item, 64) for item in value)
    if any(domain not in AUTONOMOUS_DOMAIN_NAMES for domain in result):
        raise ArgumentError(f"{name} contains an unsupported domain")
    if len(set(result)) != len(result):
        raise ArgumentError(f"{name} contains duplicate domains")
    return result


def _canonical_domains(values: Sequence[str]) -> tuple[str, ...]:
    index = {domain: position for position, domain in enumerate(AUTONOMOUS_DOMAIN_NAMES)}
    return tuple(sorted(values, key=lambda domain: index[domain]))


def _fraction(name: str, value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)) or not 0 <= float(value) <= 1:
        raise ArgumentError(f"{name} must be a finite fraction between zero and one")
    return round(float(value), 12)


def _bounded_count(name: str, value: Any, maximum: int = 64) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0 or value > maximum:
        raise ArgumentError(f"{name} is outside its bounded count contract")
    return value


def _alignment_pair(left: str, right: str) -> str:
    return f"{left}{_ALIGNMENT_PAIR_SEPARATOR}{right}"


def _exact_keys(name: str, value: Mapping[str, Any], allowed: Sequence[str]) -> None:
    if set(value) != set(allowed):
        raise ArgumentError(f"{name} contains unsupported or missing fields")


@dataclass(frozen=True, slots=True)
class AutonomousCrossDomainResponseAlignment:
    alignment_id: str
    left_domain: str
    right_domain: str
    stance: str
    confidence: float
    topic_digest: str
    rationale_digest: str | None
    left_response_digest: str
    right_response_digest: str
    alignment_digest: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ALIGNMENT_SCHEMA,
            "alignment_id": self.alignment_id,
            "left_domain": self.left_domain,
            "right_domain": self.right_domain,
            "stance": self.stance,
            "confidence": self.confidence,
            "topic_digest": self.topic_digest,
            "rationale_digest": self.rationale_digest,
            "left_response_digest": self.left_response_digest,
            "right_response_digest": self.right_response_digest,
            "alignment_digest": self.alignment_digest,
        }


@dataclass(frozen=True, slots=True)
class AutonomousCrossDomainResponseRow:
    domain: str
    role: str
    workflow_id: str
    contract_digest: str
    response_digest: str
    evaluation_digest: str
    response_status: str
    reward: float
    passed: bool
    missing_signals: tuple[str, ...]
    signals: Mapping[str, float]
    stage_status_counts: Mapping[str, int]
    domain_detail_coverage: float
    uncertainty_count: int
    evidence_gap_count: int
    next_action_count: int
    answer_digest: str
    row_digest: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ROW_SCHEMA,
            "domain": self.domain,
            "role": self.role,
            "workflow_id": self.workflow_id,
            "contract_digest": self.contract_digest,
            "response_digest": self.response_digest,
            "evaluation_digest": self.evaluation_digest,
            "response_status": self.response_status,
            "reward": self.reward,
            "passed": self.passed,
            "missing_signals": list(self.missing_signals),
            "signals": dict(self.signals),
            "stage_status_counts": dict(self.stage_status_counts),
            "domain_detail_coverage": self.domain_detail_coverage,
            "uncertainty_count": self.uncertainty_count,
            "evidence_gap_count": self.evidence_gap_count,
            "next_action_count": self.next_action_count,
            "answer_digest": self.answer_digest,
            "row_digest": self.row_digest,
        }


@dataclass(frozen=True, slots=True)
class AutonomousCrossDomainResponseAssessment:
    context_digest: str | None
    requested_domains: tuple[str, ...]
    specialist_domains: tuple[str, ...]
    present_domains: tuple[str, ...]
    missing_domains: tuple[str, ...]
    unexpected_domains: tuple[str, ...]
    rows: tuple[AutonomousCrossDomainResponseRow, ...]
    alignments: tuple[AutonomousCrossDomainResponseAlignment, ...]
    alignment_pairs_expected: int
    alignment_pairs_observed: int
    missing_alignment_pairs: tuple[str, ...]
    contradictory_alignment_ids: tuple[str, ...]
    unresolved_alignment_ids: tuple[str, ...]
    low_confidence_alignment_ids: tuple[str, ...]
    synthesis_domain_present: bool
    synthesis_response_digest: str | None
    synthesis_evaluation_digest: str | None
    require_synthesis: bool
    require_complete_alignment: bool
    minimum_reward: float
    minimum_alignment_confidence: float
    contradiction_confidence_threshold: float
    status: str
    ready_to_synthesize: bool
    gate_reasons: tuple[str, ...]
    next_actions: tuple[str, ...]
    retention: str
    evaluator_authority: str
    secret_material: str
    assessment_digest: str

    def to_dict(self) -> dict[str, Any]:
        descriptor = {
            "schema": AUTONOMOUS_CROSS_DOMAIN_RESPONSE_SCHEMA,
            "context_digest": self.context_digest,
            "requested_domains": list(self.requested_domains),
            "specialist_domains": list(self.specialist_domains),
            "present_domains": list(self.present_domains),
            "missing_domains": list(self.missing_domains),
            "unexpected_domains": list(self.unexpected_domains),
            "rows": [row.to_dict() for row in self.rows],
            "alignments": [alignment.to_dict() for alignment in self.alignments],
            "alignment_pairs_expected": self.alignment_pairs_expected,
            "alignment_pairs_observed": self.alignment_pairs_observed,
            "missing_alignment_pairs": list(self.missing_alignment_pairs),
            "contradictory_alignment_ids": list(self.contradictory_alignment_ids),
            "unresolved_alignment_ids": list(self.unresolved_alignment_ids),
            "low_confidence_alignment_ids": list(self.low_confidence_alignment_ids),
            "synthesis_domain_present": self.synthesis_domain_present,
            "synthesis_response_digest": self.synthesis_response_digest,
            "synthesis_evaluation_digest": self.synthesis_evaluation_digest,
            "require_synthesis": self.require_synthesis,
            "require_complete_alignment": self.require_complete_alignment,
            "minimum_reward": self.minimum_reward,
            "minimum_alignment_confidence": self.minimum_alignment_confidence,
            "contradiction_confidence_threshold": self.contradiction_confidence_threshold,
            "status": self.status,
            "ready_to_synthesize": self.ready_to_synthesize,
            "gate_reasons": list(self.gate_reasons),
            "next_actions": list(self.next_actions),
            "retention": self.retention,
            "evaluator_authority": self.evaluator_authority,
            "secret_material": self.secret_material,
        }
        return {**descriptor, "assessment_digest": self.assessment_digest}


def _row_descriptor(row: Mapping[str, Any]) -> dict[str, Any]:
    return {key: value for key, value in row.items() if key != "row_digest"}


def _alignment_descriptor(alignment: Mapping[str, Any]) -> dict[str, Any]:
    return {key: value for key, value in alignment.items() if key != "alignment_digest"}


def _assessment_descriptor(value: Mapping[str, Any]) -> dict[str, Any]:
    return {key: value for key, value in value.items() if key != "assessment_digest"}


def _normalize_entry(entry: Any) -> tuple[str, str, AutonomousDomainResponseContract, Any, AutonomousDomainResponseEvaluation]:
    if not isinstance(entry, Mapping):
        raise ArgumentError("cross-domain response entries must be mappings")
    _exact_keys("cross-domain response entry", entry, ("domain", "contract", "response", "role"))
    domain = _bounded_text("cross-domain response entry domain", entry.get("domain"), 64)
    if domain not in AUTONOMOUS_DOMAIN_NAMES:
        raise ArgumentError("cross-domain response entry domain is unsupported")
    role = _bounded_text("cross-domain response entry role", entry.get("role"), 32)
    if role not in AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ROLES:
        raise ArgumentError("cross-domain response entry role is unsupported")
    contract = entry.get("contract")
    if not isinstance(contract, AutonomousDomainResponseContract):
        raise ArgumentError("cross-domain response entry contract must be a reviewed response contract")
    if contract.domain != domain:
        raise ArgumentError("cross-domain response entry domain does not match its contract")
    response = validate_autonomous_domain_response(entry.get("response"), contract)
    evaluation = evaluate_autonomous_domain_response(response.to_dict(), contract)
    if domain == "cross_domain" and role != "synthesis":
        raise ArgumentError("cross_domain response entries must use the synthesis role")
    if domain != "cross_domain" and role != "specialist":
        raise ArgumentError("non-cross-domain response entries must use the specialist role")
    return domain, role, contract, response, evaluation


def _response_row(
    domain: str,
    role: str,
    contract: AutonomousDomainResponseContract,
    response: Any,
    evaluation: AutonomousDomainResponseEvaluation,
) -> AutonomousCrossDomainResponseRow:
    stage_statuses = (stage.status for stage in response.stages)
    stage_counts = {status: 0 for status in ("complete", "partial", "blocked", "not_attempted")}
    for status in stage_statuses:
        stage_counts[status] = stage_counts.get(status, 0) + 1
    signals = {name: round(float(score), 12) for name, score in evaluation.signals.items()}
    descriptor = {
        "schema": AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ROW_SCHEMA,
        "domain": domain,
        "role": role,
        "workflow_id": contract.workflow_id,
        "contract_digest": contract.contract_digest,
        "response_digest": evaluation.response_digest,
        "evaluation_digest": evaluation.evaluation_digest,
        "response_status": response.status,
        "reward": evaluation.reward,
        "passed": evaluation.passed,
        "missing_signals": list(evaluation.missing_signals),
        "signals": signals,
        "stage_status_counts": stage_counts,
        "domain_detail_coverage": signals.get("domain_detail_coverage", 0.0),
        "uncertainty_count": len(response.uncertainty),
        "evidence_gap_count": len(response.evidence_gaps),
        "next_action_count": len(response.next_actions),
        "answer_digest": content_digest({"answer": response.answer}),
    }
    return AutonomousCrossDomainResponseRow(
        row_digest=content_digest(descriptor),
        **{key: value for key, value in descriptor.items() if key != "schema"},
    )


def _normalize_alignment(
    value: Any,
    rows: Mapping[str, AutonomousCrossDomainResponseRow],
    *,
    order: Mapping[str, int],
) -> AutonomousCrossDomainResponseAlignment:
    if not isinstance(value, Mapping):
        raise ArgumentError("cross-domain alignments must be mappings")
    _exact_keys(
        "cross-domain alignment",
        value,
        ("alignment_id", "left_domain", "right_domain", "stance", "confidence", "topic_digest", "rationale_digest", "left_response_digest", "right_response_digest"),
    )
    alignment_id = _identifier("cross-domain alignment id", value.get("alignment_id"))
    left = _bounded_text("cross-domain alignment left domain", value.get("left_domain"), 64)
    right = _bounded_text("cross-domain alignment right domain", value.get("right_domain"), 64)
    if left == right:
        raise ArgumentError("cross-domain alignment cannot compare a domain with itself")
    if left not in rows or right not in rows:
        raise ArgumentError("cross-domain alignment domains must have response rows")
    if order[left] > order[right]:
        left, right = right, left
        left_digest, right_digest = value.get("right_response_digest"), value.get("left_response_digest")
    else:
        left_digest, right_digest = value.get("left_response_digest"), value.get("right_response_digest")
    left_response_digest = _digest("cross-domain alignment left response digest", left_digest)
    right_response_digest = _digest("cross-domain alignment right response digest", right_digest)
    if left_response_digest != rows[left].response_digest or right_response_digest != rows[right].response_digest:
        raise ArgumentError("cross-domain alignment response digests do not match the reviewed rows")
    stance = _bounded_text("cross-domain alignment stance", value.get("stance"), 32)
    if stance not in AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ALIGNMENT_STANCES:
        raise ArgumentError("cross-domain alignment stance is unsupported")
    confidence = _fraction("cross-domain alignment confidence", value.get("confidence"))
    topic_digest = _digest("cross-domain alignment topic digest", value.get("topic_digest"))
    rationale_digest = _optional_digest("cross-domain alignment rationale digest", value.get("rationale_digest"))
    descriptor = {
        "schema": AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ALIGNMENT_SCHEMA,
        "alignment_id": alignment_id,
        "left_domain": left,
        "right_domain": right,
        "stance": stance,
        "confidence": confidence,
        "topic_digest": topic_digest,
        "rationale_digest": rationale_digest,
        "left_response_digest": left_response_digest,
        "right_response_digest": right_response_digest,
    }
    return AutonomousCrossDomainResponseAlignment(
        alignment_digest=content_digest(descriptor),
        **{key: value for key, value in descriptor.items() if key != "schema"},
    )


def _gate_actions(
    *,
    missing_domains: Sequence[str],
    blocked: bool,
    weak_rows: bool,
    missing_alignment_pairs: Sequence[str],
    contradictions: Sequence[str],
    unresolved: Sequence[str],
    low_confidence: Sequence[str],
    synthesis_missing: bool,
    completed: bool,
) -> tuple[str, ...]:
    actions: list[str] = []
    if missing_domains:
        actions.append("acquire_missing_domain_responses")
    if blocked:
        actions.append("review_blocked_domain_response")
    if weak_rows:
        actions.append("repair_domain_response_integrity")
    if missing_alignment_pairs:
        actions.append("perform_pairwise_cross_domain_alignment")
    if contradictions:
        actions.append("resolve_cross_domain_contradiction")
    if unresolved:
        actions.append("review_unresolved_cross_domain_alignment")
    if low_confidence:
        actions.append("review_low_confidence_cross_domain_alignment")
    if synthesis_missing:
        actions.append("run_cross_domain_synthesis")
    if completed:
        return ()
    if not actions:
        actions.append("review_cross_domain_synthesis_gate")
    return tuple(actions[:MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ACTIONS])


def assess_autonomous_cross_domain_response_set(
    responses: Sequence[Mapping[str, Any]],
    *,
    requested_domains: Sequence[str] | None = None,
    context_digest: str | None = None,
    alignments: Sequence[Mapping[str, Any]] = (),
    require_synthesis: bool = False,
    require_complete_alignment: bool = True,
    minimum_reward: float = AUTONOMOUS_CROSS_DOMAIN_RESPONSE_MIN_REWARD,
    minimum_alignment_confidence: float = AUTONOMOUS_CROSS_DOMAIN_RESPONSE_MIN_ALIGNMENT_CONFIDENCE,
    contradiction_confidence_threshold: float = AUTONOMOUS_CROSS_DOMAIN_RESPONSE_CONTRADICTION_CONFIDENCE,
) -> AutonomousCrossDomainResponseAssessment:
    """Assess specialist responses before a cross-domain synthesis call.

    The function is provider-free.  It accepts transient response values, validates and scores
    them immediately, and returns a digest-only assessment.  Alignments are caller-owned semantic
    review metadata, not proof of agreement or contradiction in the external world.
    """

    if isinstance(responses, (str, bytes, bytearray)) or not isinstance(responses, Sequence) or not 1 <= len(responses) <= MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ENTRIES:
        raise ArgumentError("cross-domain response entries are outside their bound")
    if not isinstance(require_synthesis, bool) or not isinstance(require_complete_alignment, bool):
        raise ArgumentError("cross-domain response gate controls must be booleans")
    minimum_reward = _fraction("cross-domain minimum reward", minimum_reward)
    minimum_alignment_confidence = _fraction("cross-domain minimum alignment confidence", minimum_alignment_confidence)
    contradiction_confidence_threshold = _fraction("cross-domain contradiction confidence threshold", contradiction_confidence_threshold)
    if context_digest is not None:
        context_digest = _digest("cross-domain response context digest", context_digest)

    normalized_entries = [_normalize_entry(entry) for entry in responses]
    domains_seen: set[str] = set()
    rows: list[AutonomousCrossDomainResponseRow] = []
    for domain, role, contract, response, evaluation in normalized_entries:
        if domain in domains_seen:
            raise ArgumentError(f"cross-domain response domain {domain} is duplicated")
        domains_seen.add(domain)
        rows.append(_response_row(domain, role, contract, response, evaluation))
    rows.sort(key=lambda row: AUTONOMOUS_DOMAIN_NAMES.index(row.domain))
    row_map = {row.domain: row for row in rows}

    if requested_domains is None:
        requested = _canonical_domains(tuple(domain for domain in domains_seen if domain != "cross_domain"))
        if len(requested) < 2:
            raise ArgumentError("cross-domain response assessment requires at least two specialist domains")
    else:
        requested = _canonical_domains(_domains("cross-domain requested domains", requested_domains))
        specialist_requested = tuple(domain for domain in requested if domain != "cross_domain")
        if len(specialist_requested) < 2:
            raise ArgumentError("cross-domain response assessment requires at least two specialist domains")
    specialist_domains = tuple(domain for domain in requested if domain != "cross_domain")
    present_domains = tuple(row.domain for row in rows)
    missing_domains = tuple(domain for domain in requested if domain not in row_map)
    unexpected_domains = tuple(domain for domain in present_domains if domain not in requested and domain != "cross_domain")
    if unexpected_domains:
        raise ArgumentError("cross-domain response entries include domains outside the requested review set")

    raw_alignments = () if alignments is None else alignments
    if isinstance(raw_alignments, (str, bytes, bytearray)) or not isinstance(raw_alignments, Sequence) or len(raw_alignments) > MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ALIGNMENTS:
        raise ArgumentError("cross-domain alignments are outside their bound")
    order = {domain: index for index, domain in enumerate(AUTONOMOUS_DOMAIN_NAMES)}
    normalized_alignments: list[AutonomousCrossDomainResponseAlignment] = []
    alignment_ids: set[str] = set()
    for raw_alignment in raw_alignments:
        raw_alignment_without_schema = dict(raw_alignment)
        raw_alignment_without_schema.pop("schema", None)
        raw_alignment_without_schema.pop("alignment_digest", None)
        alignment = _normalize_alignment(raw_alignment_without_schema, row_map, order=order)
        if alignment.alignment_id in alignment_ids:
            raise ArgumentError("cross-domain alignment ids must be unique")
        alignment_ids.add(alignment.alignment_id)
        normalized_alignments.append(alignment)
    normalized_alignments.sort(key=lambda item: item.alignment_id)

    present_specialists = tuple(domain for domain in specialist_domains if domain in row_map)
    expected_pairs = len(present_specialists) * (len(present_specialists) - 1) // 2
    observed_pairs = {
        _alignment_pair(alignment.left_domain, alignment.right_domain)
        for alignment in normalized_alignments
        if alignment.left_domain in present_specialists and alignment.right_domain in present_specialists
    }
    all_pairs = {
        _alignment_pair(left, right)
        for left_index, left in enumerate(present_specialists)
        for right in present_specialists[left_index + 1:]
    }
    missing_alignment_pairs = tuple(sorted(all_pairs.difference(observed_pairs))) if require_complete_alignment else ()
    contradictory = tuple(sorted(
        alignment.alignment_id
        for alignment in normalized_alignments
        if alignment.stance == "contradict" and alignment.confidence >= contradiction_confidence_threshold
    ))
    unresolved = tuple(sorted(
        alignment.alignment_id
        for alignment in normalized_alignments
        if alignment.stance == "unresolved" and alignment.confidence >= minimum_alignment_confidence
    ))
    low_confidence = tuple(sorted(
        alignment.alignment_id
        for alignment in normalized_alignments
        if alignment.confidence < minimum_alignment_confidence
    ))

    blocked = any(row.response_status == "blocked" or any(count > 0 for status, count in row.stage_status_counts.items() if status == "blocked") for row in rows)
    weak_rows = any(row.response_status != "complete" or not row.passed or row.reward < minimum_reward for row in rows if row.role == "specialist")
    synthesis_row = row_map.get("cross_domain")
    synthesis_missing = require_synthesis and synthesis_row is None
    synthesis_weak = require_synthesis and synthesis_row is not None and (
        synthesis_row.response_status != "complete" or not synthesis_row.passed or synthesis_row.reward < minimum_reward
    )
    gate_reasons: list[str] = []
    if missing_domains:
        gate_reasons.append("missing_domain_coverage")
    if unexpected_domains:
        gate_reasons.append("unexpected_domain_coverage")
    if blocked:
        gate_reasons.append("blocked_domain_response")
    if weak_rows:
        gate_reasons.append("domain_response_integrity_below_threshold")
    if synthesis_missing:
        gate_reasons.append("synthesis_response_missing")
    if synthesis_weak:
        gate_reasons.append("synthesis_response_integrity_below_threshold")
    if missing_alignment_pairs:
        gate_reasons.append("pairwise_alignment_incomplete")
    if contradictory:
        gate_reasons.append("high_confidence_contradiction")
    if unresolved:
        gate_reasons.append("unresolved_alignment")
    if low_confidence:
        gate_reasons.append("low_confidence_alignment")
    alignment_reasons = {"pairwise_alignment_incomplete", "high_confidence_contradiction", "unresolved_alignment", "low_confidence_alignment"}
    alignment_only = bool(gate_reasons) and set(gate_reasons).issubset(alignment_reasons)
    has_material_failure = bool(set(gate_reasons) - alignment_reasons)
    completed = not gate_reasons and synthesis_row is not None
    if blocked:
        status = "blocked"
    elif has_material_failure:
        status = "partial"
    elif alignment_only:
        status = "needs_alignment_review"
    elif completed:
        status = "completed"
    else:
        status = "ready_to_synthesize"
    ready = status == "ready_to_synthesize"
    if completed:
        ready = False
    next_actions = _gate_actions(
        missing_domains=missing_domains,
        blocked=blocked,
        weak_rows=weak_rows or synthesis_weak,
        missing_alignment_pairs=missing_alignment_pairs,
        contradictions=contradictory,
        unresolved=unresolved,
        low_confidence=low_confidence,
        synthesis_missing=synthesis_missing,
        completed=completed,
    )
    descriptor = {
        "schema": AUTONOMOUS_CROSS_DOMAIN_RESPONSE_SCHEMA,
        "context_digest": context_digest,
        "requested_domains": list(requested),
        "specialist_domains": list(specialist_domains),
        "present_domains": list(present_domains),
        "missing_domains": list(missing_domains),
        "unexpected_domains": list(unexpected_domains),
        "rows": [row.to_dict() for row in rows],
        "alignments": [alignment.to_dict() for alignment in normalized_alignments],
        "alignment_pairs_expected": expected_pairs,
        "alignment_pairs_observed": len(observed_pairs),
        "missing_alignment_pairs": list(missing_alignment_pairs),
        "contradictory_alignment_ids": list(contradictory),
        "unresolved_alignment_ids": list(unresolved),
        "low_confidence_alignment_ids": list(low_confidence),
        "synthesis_domain_present": synthesis_row is not None,
        "synthesis_response_digest": None if synthesis_row is None else synthesis_row.response_digest,
        "synthesis_evaluation_digest": None if synthesis_row is None else synthesis_row.evaluation_digest,
        "require_synthesis": require_synthesis,
        "require_complete_alignment": require_complete_alignment,
        "minimum_reward": minimum_reward,
        "minimum_alignment_confidence": minimum_alignment_confidence,
        "contradiction_confidence_threshold": contradiction_confidence_threshold,
        "status": status,
        "ready_to_synthesize": ready,
        "gate_reasons": list(gate_reasons),
        "next_actions": list(next_actions),
        "retention": _RETENTION,
        "evaluator_authority": _AUTHORITY,
        "secret_material": "never_returned",
    }
    assessment_digest = content_digest(descriptor)
    result = AutonomousCrossDomainResponseAssessment(
        context_digest=context_digest,
        requested_domains=requested,
        specialist_domains=specialist_domains,
        present_domains=present_domains,
        missing_domains=missing_domains,
        unexpected_domains=unexpected_domains,
        rows=tuple(rows),
        alignments=tuple(normalized_alignments),
        alignment_pairs_expected=expected_pairs,
        alignment_pairs_observed=len(observed_pairs),
        missing_alignment_pairs=missing_alignment_pairs,
        contradictory_alignment_ids=contradictory,
        unresolved_alignment_ids=unresolved,
        low_confidence_alignment_ids=low_confidence,
        synthesis_domain_present=synthesis_row is not None,
        synthesis_response_digest=None if synthesis_row is None else synthesis_row.response_digest,
        synthesis_evaluation_digest=None if synthesis_row is None else synthesis_row.evaluation_digest,
        require_synthesis=require_synthesis,
        require_complete_alignment=require_complete_alignment,
        minimum_reward=minimum_reward,
        minimum_alignment_confidence=minimum_alignment_confidence,
        contradiction_confidence_threshold=contradiction_confidence_threshold,
        status=status,
        ready_to_synthesize=ready,
        gate_reasons=tuple(gate_reasons),
        next_actions=next_actions,
        retention=_RETENTION,
        evaluator_authority=_AUTHORITY,
        secret_material="never_returned",
        assessment_digest=assessment_digest,
    )
    if _json_bytes(result.to_dict()) > MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_BYTES:
        raise ArgumentError("cross-domain response assessment exceeds its byte bound")
    return result


def validate_autonomous_cross_domain_response_assessment(value: Any) -> AutonomousCrossDomainResponseAssessment:
    """Validate a digest-only cross-domain gate projection before persistence or learning."""

    if not isinstance(value, Mapping):
        raise ArgumentError("cross-domain response assessment must be a mapping")
    _assert_safe_metadata(value)
    allowed = {
        "schema", "context_digest", "requested_domains", "specialist_domains", "present_domains", "missing_domains",
        "unexpected_domains", "rows", "alignments", "alignment_pairs_expected", "alignment_pairs_observed",
        "missing_alignment_pairs", "contradictory_alignment_ids", "unresolved_alignment_ids", "low_confidence_alignment_ids",
        "synthesis_domain_present", "synthesis_response_digest", "synthesis_evaluation_digest", "require_synthesis",
        "require_complete_alignment", "minimum_reward", "minimum_alignment_confidence", "contradiction_confidence_threshold",
        "status", "ready_to_synthesize", "gate_reasons", "next_actions", "retention", "evaluator_authority", "secret_material", "assessment_digest",
    }
    if set(value) != allowed:
        raise ArgumentError("cross-domain response assessment contains unsupported or missing fields")
    if value.get("schema") != AUTONOMOUS_CROSS_DOMAIN_RESPONSE_SCHEMA:
        raise ArgumentError("cross-domain response assessment schema is invalid")
    if value.get("retention") != _RETENTION or value.get("evaluator_authority") != _AUTHORITY or value.get("secret_material") != "never_returned":
        raise ArgumentError("cross-domain response assessment retention contract is invalid")
    context_digest = _optional_digest("cross-domain response context digest", value.get("context_digest"))
    requested = _canonical_domains(_domains("cross-domain response requested domains", value.get("requested_domains")))
    specialist = tuple(domain for domain in requested if domain != "cross_domain")
    if len(specialist) < 2 or tuple(value.get("specialist_domains", ())) != specialist:
        raise ArgumentError("cross-domain response specialist domain projection is inconsistent")
    present = _canonical_domains(_domains("cross-domain response present domains", value.get("present_domains"), minimum=1))
    missing = tuple(value.get("missing_domains", ()))
    unexpected = tuple(value.get("unexpected_domains", ()))
    if tuple(value.get("missing_domains", ())) != tuple(domain for domain in requested if domain not in present):
        raise ArgumentError("cross-domain response missing domain projection is inconsistent")
    if tuple(value.get("unexpected_domains", ())) != tuple(domain for domain in present if domain not in requested and domain != "cross_domain"):
        raise ArgumentError("cross-domain response unexpected domain projection is inconsistent")
    rows_raw = value.get("rows")
    if isinstance(rows_raw, (str, bytes, bytearray)) or not isinstance(rows_raw, Sequence) or not 1 <= len(rows_raw) <= MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ENTRIES:
        raise ArgumentError("cross-domain response rows are outside their bound")
    rows: list[AutonomousCrossDomainResponseRow] = []
    row_domains: set[str] = set()
    for raw in rows_raw:
        if not isinstance(raw, Mapping):
            raise ArgumentError("cross-domain response row is malformed")
        allowed_row = {
            "schema", "domain", "role", "workflow_id", "contract_digest", "response_digest", "evaluation_digest",
            "response_status", "reward", "passed", "missing_signals", "signals", "stage_status_counts",
            "domain_detail_coverage", "uncertainty_count", "evidence_gap_count", "next_action_count", "answer_digest", "row_digest",
        }
        if set(raw) != allowed_row or raw.get("schema") != AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ROW_SCHEMA:
            raise ArgumentError("cross-domain response row has unsupported or missing fields")
        domain = _bounded_text("cross-domain response row domain", raw.get("domain"), 64)
        if domain not in AUTONOMOUS_DOMAIN_NAMES or domain in row_domains:
            raise ArgumentError("cross-domain response row domain is invalid or duplicated")
        row_domains.add(domain)
        role = _bounded_text("cross-domain response row role", raw.get("role"), 32)
        if role not in AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ROLES:
            raise ArgumentError("cross-domain response row role is invalid")
        descriptor = dict(raw)
        row_digest = _digest("cross-domain response row digest", descriptor.pop("row_digest"))
        if content_digest(descriptor) != row_digest:
            raise ArgumentError("cross-domain response row digest does not match its projection")
        signals = raw.get("signals")
        if not isinstance(signals, Mapping) or not signals:
            raise ArgumentError("cross-domain response row signals are malformed")
        normalized_signals = {str(key): _fraction(f"cross-domain response signal {key}", score) for key, score in signals.items()}
        stage_counts = raw.get("stage_status_counts")
        if not isinstance(stage_counts, Mapping):
            raise ArgumentError("cross-domain response row stage status counts are malformed")
        normalized_counts: dict[str, int] = {}
        for key, count in stage_counts.items():
            if not isinstance(key, str) or not isinstance(count, int) or isinstance(count, bool) or count < 0 or count > 64:
                raise ArgumentError("cross-domain response row stage status count is invalid")
            normalized_counts[key] = count
        reward = _fraction("cross-domain response row reward", raw.get("reward"))
        if not isinstance(raw.get("passed"), bool):
            raise ArgumentError("cross-domain response row passed flag is invalid")
        uncertainty_count = _bounded_count("cross-domain response row uncertainty count", raw.get("uncertainty_count"))
        evidence_gap_count = _bounded_count("cross-domain response row evidence gap count", raw.get("evidence_gap_count"))
        next_action_count = _bounded_count("cross-domain response row next action count", raw.get("next_action_count"))
        rows.append(AutonomousCrossDomainResponseRow(
            domain=domain,
            role=role,
            workflow_id=_identifier("cross-domain response row workflow id", raw.get("workflow_id")),
            contract_digest=_digest("cross-domain response row contract digest", raw.get("contract_digest")),
            response_digest=_digest("cross-domain response row response digest", raw.get("response_digest")),
            evaluation_digest=_digest("cross-domain response row evaluation digest", raw.get("evaluation_digest")),
            response_status=_bounded_text("cross-domain response row status", raw.get("response_status"), 64),
            reward=reward,
            passed=raw["passed"],
            missing_signals=_bounded_strings("cross-domain response row missing signals", raw.get("missing_signals")),
            signals=normalized_signals,
            stage_status_counts=normalized_counts,
            domain_detail_coverage=_fraction("cross-domain response row detail coverage", raw.get("domain_detail_coverage")),
            uncertainty_count=uncertainty_count,
            evidence_gap_count=evidence_gap_count,
            next_action_count=next_action_count,
            answer_digest=_digest("cross-domain response row answer digest", raw.get("answer_digest")),
            row_digest=row_digest,
        ))
    if tuple(row.domain for row in sorted(rows, key=lambda row: AUTONOMOUS_DOMAIN_NAMES.index(row.domain))) != present:
        raise ArgumentError("cross-domain response row domains do not match the present domain projection")
    alignments_raw = value.get("alignments")
    if isinstance(alignments_raw, (str, bytes, bytearray)) or not isinstance(alignments_raw, Sequence) or len(alignments_raw) > MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ALIGNMENTS:
        raise ArgumentError("cross-domain response alignments are outside their bound")
    alignment_ids: set[str] = set()
    alignments: list[AutonomousCrossDomainResponseAlignment] = []
    row_map = {row.domain: row for row in rows}
    order = {domain: index for index, domain in enumerate(AUTONOMOUS_DOMAIN_NAMES)}
    for raw_alignment in alignments_raw:
        raw_alignment_without_schema = dict(raw_alignment)
        raw_alignment_without_schema.pop("schema", None)
        raw_alignment_without_schema.pop("alignment_digest", None)
        alignment = _normalize_alignment(raw_alignment_without_schema, row_map, order=order)
        if alignment.alignment_id in alignment_ids:
            raise ArgumentError("cross-domain response alignment ids are duplicated")
        alignment_ids.add(alignment.alignment_id)
        alignments.append(alignment)
    if [alignment.to_dict() for alignment in alignments] != list(alignments_raw):
        raise ArgumentError("cross-domain response alignments are not canonically ordered or normalized")
    expected = value.get("alignment_pairs_expected")
    observed = value.get("alignment_pairs_observed")
    if not isinstance(expected, int) or isinstance(expected, bool) or expected < 0 or not isinstance(observed, int) or isinstance(observed, bool) or observed < 0 or observed > expected:
        raise ArgumentError("cross-domain response alignment pair counts are invalid")
    require_synthesis = value.get("require_synthesis")
    require_complete_alignment = value.get("require_complete_alignment")
    if not isinstance(require_synthesis, bool) or not isinstance(require_complete_alignment, bool):
        raise ArgumentError("cross-domain response assessment gate controls are invalid")
    minimum_reward = _fraction("cross-domain assessment minimum reward", value.get("minimum_reward"))
    minimum_alignment_confidence = _fraction("cross-domain assessment minimum alignment confidence", value.get("minimum_alignment_confidence"))
    contradiction_threshold = _fraction("cross-domain assessment contradiction threshold", value.get("contradiction_confidence_threshold"))
    status = _bounded_text("cross-domain response assessment status", value.get("status"), 64)
    if status not in AUTONOMOUS_CROSS_DOMAIN_RESPONSE_STATUSES or not isinstance(value.get("ready_to_synthesize"), bool):
        raise ArgumentError("cross-domain response assessment status is invalid")
    for name in ("synthesis_domain_present",):
        if not isinstance(value.get(name), bool):
            raise ArgumentError(f"cross-domain response assessment {name} is invalid")
    synthesis_response_digest = _optional_digest("cross-domain synthesis response digest", value.get("synthesis_response_digest"))
    synthesis_evaluation_digest = _optional_digest("cross-domain synthesis evaluation digest", value.get("synthesis_evaluation_digest"))
    if (value["synthesis_domain_present"] is False) != (synthesis_response_digest is None and synthesis_evaluation_digest is None):
        raise ArgumentError("cross-domain synthesis digest presence is inconsistent")
    gate_reasons = _bounded_strings("cross-domain response gate reasons", value.get("gate_reasons"), MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_REASONS)
    next_actions = _bounded_strings("cross-domain response next actions", value.get("next_actions"))
    descriptor = dict(value)
    assessment_digest = _digest("cross-domain response assessment digest", descriptor.pop("assessment_digest"))
    if content_digest(descriptor) != assessment_digest:
        raise ArgumentError("cross-domain response assessment digest does not match its projection")
    if _json_bytes(value) > MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_BYTES:
        raise ArgumentError("cross-domain response assessment exceeds its byte bound")
    return AutonomousCrossDomainResponseAssessment(
        context_digest=context_digest,
        requested_domains=requested,
        specialist_domains=specialist,
        present_domains=present,
        missing_domains=missing,
        unexpected_domains=unexpected,
        rows=tuple(sorted(rows, key=lambda row: AUTONOMOUS_DOMAIN_NAMES.index(row.domain))),
        alignments=tuple(alignments),
        alignment_pairs_expected=expected,
        alignment_pairs_observed=observed,
        missing_alignment_pairs=tuple(value["missing_alignment_pairs"]),
        contradictory_alignment_ids=tuple(value["contradictory_alignment_ids"]),
        unresolved_alignment_ids=tuple(value["unresolved_alignment_ids"]),
        low_confidence_alignment_ids=tuple(value["low_confidence_alignment_ids"]),
        synthesis_domain_present=value["synthesis_domain_present"],
        synthesis_response_digest=synthesis_response_digest,
        synthesis_evaluation_digest=synthesis_evaluation_digest,
        require_synthesis=require_synthesis,
        require_complete_alignment=require_complete_alignment,
        minimum_reward=minimum_reward,
        minimum_alignment_confidence=minimum_alignment_confidence,
        contradiction_confidence_threshold=contradiction_threshold,
        status=status,
        ready_to_synthesize=value["ready_to_synthesize"],
        gate_reasons=gate_reasons,
        next_actions=next_actions,
        retention=_RETENTION,
        evaluator_authority=_AUTHORITY,
        secret_material="never_returned",
        assessment_digest=assessment_digest,
    )


def replay_autonomous_cross_domain_response_assessment(
    responses: Sequence[Mapping[str, Any]],
    expected: AutonomousCrossDomainResponseAssessment | Mapping[str, Any],
    **options: Any,
) -> AutonomousCrossDomainResponseAssessment:
    """Recompute the gate and reject drift from a persisted digest-only assessment."""

    expected_assessment = expected if isinstance(expected, AutonomousCrossDomainResponseAssessment) else validate_autonomous_cross_domain_response_assessment(expected)
    replayed = assess_autonomous_cross_domain_response_set(responses, **options)
    if replayed.assessment_digest != expected_assessment.assessment_digest:
        raise ArgumentError("cross-domain response assessment replay drifted from the recorded projection")
    return replayed


__all__ = [
    "AUTONOMOUS_CROSS_DOMAIN_RESPONSE_SCHEMA",
    "AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ROW_SCHEMA",
    "AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ALIGNMENT_SCHEMA",
    "AUTONOMOUS_CROSS_DOMAIN_RESPONSE_STATUSES",
    "AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ROLES",
    "AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ALIGNMENT_STANCES",
    "MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ENTRIES",
    "MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ALIGNMENTS",
    "MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_ACTIONS",
    "MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_REASONS",
    "MAX_AUTONOMOUS_CROSS_DOMAIN_RESPONSE_BYTES",
    "AUTONOMOUS_CROSS_DOMAIN_RESPONSE_MIN_REWARD",
    "AUTONOMOUS_CROSS_DOMAIN_RESPONSE_MIN_ALIGNMENT_CONFIDENCE",
    "AUTONOMOUS_CROSS_DOMAIN_RESPONSE_CONTRADICTION_CONFIDENCE",
    "AutonomousCrossDomainResponseAlignment",
    "AutonomousCrossDomainResponseRow",
    "AutonomousCrossDomainResponseAssessment",
    "assess_autonomous_cross_domain_response_set",
    "validate_autonomous_cross_domain_response_assessment",
    "replay_autonomous_cross_domain_response_assessment",
]
