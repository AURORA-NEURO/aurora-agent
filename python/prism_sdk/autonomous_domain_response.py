"""Structured, domain-aware provider response contracts.

The autonomous router can choose a domain, workflow, model, and tool surface, but a generic
``{"answer": "..."}`` response is not enough to make the resulting decision useful.  This module
defines the opt-in response contract shared by every built-in domain.  It is deliberately a
structural evaluator: it can measure whether the provider reported the reviewed stages,
uncertainty, evidence gaps, and domain-specific fields, but it cannot establish external truth.

Provider values are validated in memory and returned to the caller.  Durable projections contain
only the response digest and structural scores; credentials and raw provider content never enter
the contract, evaluation, or replay value.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
import math
import re
from typing import Any, Mapping, Sequence

from .authoring import content_digest
from .errors import ArgumentError


AUTONOMOUS_DOMAIN_RESPONSE_SCHEMA = "bioprism-python-autonomous-domain-response/0.1"
AUTONOMOUS_DOMAIN_RESPONSE_CONTRACT_SCHEMA = "bioprism-python-autonomous-domain-response-contract/0.1"
AUTONOMOUS_DOMAIN_RESPONSE_EVALUATION_SCHEMA = "bioprism-python-autonomous-domain-response-evaluation/0.1"
AUTONOMOUS_DOMAIN_RESPONSE_STATUSES = ("complete", "partial", "blocked", "needs_review")
AUTONOMOUS_DOMAIN_STAGE_RESPONSE_STATUSES = ("complete", "partial", "blocked", "not_attempted")
MAX_AUTONOMOUS_DOMAIN_RESPONSE_ITEMS = 64
MAX_AUTONOMOUS_DOMAIN_RESPONSE_ITEM_BYTES = 8_192
MAX_AUTONOMOUS_DOMAIN_RESPONSE_ANSWER_BYTES = 64_000
MAX_AUTONOMOUS_DOMAIN_RESPONSE_CONTRACT_BYTES = 1_000_000
AUTONOMOUS_DOMAIN_RESPONSE_EVALUATOR_VERSION = "1"
AUTONOMOUS_DOMAIN_RESPONSE_PASS_THRESHOLD = 0.8

AUTONOMOUS_DOMAIN_RESPONSE_FIELDS: dict[str, tuple[str, ...]] = {
    "coding": ("files_or_components", "tests_and_verification", "residual_risks", "rollback_or_follow_up"),
    "browser": ("sources", "citations", "freshness", "retrieval_gaps"),
    "data": ("schema_and_units", "lineage", "quality_metrics", "anomalies_and_transformations"),
    "science": ("estimand_and_assumptions", "evidence_map", "hypotheses_and_predictions", "design_and_controls", "reproduction_plan"),
    "biomedical": ("scope_boundary", "provenance", "population_and_applicability", "uncertainty", "human_review_and_escalation"),
    "neuroscience": ("measurement_contract", "preprocessing_and_exclusions", "confounds", "model_sensitivity", "validation_plan"),
    "operations": ("observed_state", "blast_radius_and_stop_conditions", "rollback_and_recovery", "approval_request", "execution_boundary"),
    "enterprise": ("stakeholders_and_owners", "policy_constraints", "options_and_tradeoffs", "decision_and_approver", "audit_plan"),
    "multi_agent": ("subtasks_and_interfaces", "assignments_and_budgets", "reconciliation", "conflicts_and_dissent", "accountable_authority"),
    "multimodal": ("available_modalities", "modality_observations", "alignment", "missing_modalities", "blind_spots"),
    "cross_domain": ("domain_attributions", "terminology_and_units", "disagreements", "decision_gate", "open_questions"),
    "evaluation": ("rubric_and_pass_criteria", "cases_and_coverage", "replay_outcomes", "failures_and_regressions", "reproduction_and_next_learning"),
}

_IDENTIFIER = re.compile(r"^[A-Za-z0-9_.:-]+$")
_FIELD_IDENTIFIER = re.compile(r"^[A-Za-z0-9_.:_-]+$")
_CREDENTIAL_SHAPES = re.compile(r"\b(?:gsk_|sk-proj-|sk-[A-Za-z0-9]{16,})", re.IGNORECASE)
_SECRET_KEYS = {
    "apikey", "authorization", "bearer", "credential", "credentials", "password", "secret",
    "secretkey", "token", "accesstoken", "refreshtoken", "privatekey",
}


def _encoded_bytes(value: str) -> int:
    return len(value.encode("utf-8"))


def _text(name: str, value: Any, maximum: int) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or _encoded_bytes(value) > maximum:
        raise ArgumentError(f"{name} is outside its bounded text contract")
    return value


def _identifier(name: str, value: Any) -> str:
    value = _text(name, value, 256)
    if not _IDENTIFIER.fullmatch(value):
        raise ArgumentError(f"{name} must be a bounded identifier")
    return value


def _bounded_list(name: str, value: Any, maximum: int = MAX_AUTONOMOUS_DOMAIN_RESPONSE_ITEMS) -> tuple[str, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)) or len(value) > maximum:
        raise ArgumentError(f"{name} must contain at most {maximum} entries")
    return tuple(_text(f"{name} entry", item, MAX_AUTONOMOUS_DOMAIN_RESPONSE_ITEM_BYTES) for item in value)


def _safe_value(value: Any, depth: int = 0) -> None:
    if depth > 16:
        raise ArgumentError("domain response is too deeply nested")
    if isinstance(value, str):
        if _CREDENTIAL_SHAPES.search(value):
            raise ArgumentError("domain response contains credential-shaped material")
        return
    if isinstance(value, Mapping):
        for key, child in value.items():
            normalized = "".join(character for character in key.lower() if character.isalnum()) if isinstance(key, str) else ""
            if normalized in _SECRET_KEYS or normalized.startswith(("gsk", "skproj")):
                raise ArgumentError("domain response contains credential-shaped fields")
            _safe_value(child, depth + 1)
    elif isinstance(value, (list, tuple)):
        for child in value:
            _safe_value(child, depth + 1)


def _json_bytes(value: Any) -> int:
    try:
        return _encoded_bytes(json.dumps(value, ensure_ascii=False, separators=(",", ":"), allow_nan=False))
    except (TypeError, ValueError) as error:
        raise ArgumentError("domain response contract must be JSON-safe") from error


def _exact_keys(name: str, value: Mapping[str, Any], allowed: Sequence[str]) -> None:
    if set(value) != set(allowed):
        raise ArgumentError(f"{name} contains unsupported or missing fields")


def _fraction(total: int, satisfied: int) -> float:
    if total <= 0:
        return 0.0
    return round(max(0.0, min(1.0, satisfied / total)), 12)


@dataclass(frozen=True, slots=True)
class AutonomousDomainStageResponse:
    stage_id: str
    status: str
    evidence: tuple[str, ...]
    findings: tuple[str, ...]
    uncertainty: tuple[str, ...]
    open_questions: tuple[str, ...]

    def to_dict(self) -> dict[str, Any]:
        return {
            "stage_id": self.stage_id,
            "status": self.status,
            "evidence": list(self.evidence),
            "findings": list(self.findings),
            "uncertainty": list(self.uncertainty),
            "open_questions": list(self.open_questions),
        }


@dataclass(frozen=True, slots=True)
class AutonomousDomainResponse:
    schema: str
    domain: str
    workflow_id: str
    status: str
    answer: str
    observations: tuple[str, ...]
    inferences: tuple[str, ...]
    uncertainty: tuple[str, ...]
    evidence_gaps: tuple[str, ...]
    next_actions: tuple[str, ...]
    stages: tuple[AutonomousDomainStageResponse, ...]
    domain_details: Mapping[str, tuple[str, ...]]
    retention: str
    secret_material: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": self.schema,
            "domain": self.domain,
            "workflow_id": self.workflow_id,
            "status": self.status,
            "answer": self.answer,
            "observations": list(self.observations),
            "inferences": list(self.inferences),
            "uncertainty": list(self.uncertainty),
            "evidence_gaps": list(self.evidence_gaps),
            "next_actions": list(self.next_actions),
            "stages": [stage.to_dict() for stage in self.stages],
            "domain_details": {field: list(values) for field, values in self.domain_details.items()},
            "retention": self.retention,
            "secret_material": self.secret_material,
        }


@dataclass(frozen=True, slots=True)
class AutonomousDomainResponseContract:
    schema: str
    version: str
    domain: str
    workflow_id: str
    workflow_digest: str
    stage_ids: tuple[str, ...]
    domain_fields: tuple[str, ...]
    response_schema: Mapping[str, Any]
    prompt_contract: str
    contract_digest: str
    retention: str
    secret_material: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": self.schema,
            "version": self.version,
            "domain": self.domain,
            "workflow_id": self.workflow_id,
            "workflow_digest": self.workflow_digest,
            "stage_ids": list(self.stage_ids),
            "domain_fields": list(self.domain_fields),
            "response_schema": dict(self.response_schema),
            "prompt_contract": self.prompt_contract,
            "contract_digest": self.contract_digest,
            "retention": self.retention,
            "secret_material": self.secret_material,
        }


@dataclass(frozen=True, slots=True)
class AutonomousDomainResponseEvaluation:
    schema: str
    evaluator_id: str
    evaluator_version: str
    domain: str
    workflow_id: str
    workflow_digest: str
    contract_digest: str
    response_digest: str
    signals: Mapping[str, float]
    missing_signals: tuple[str, ...]
    reward: float
    passed: bool
    failed: bool
    failure_class: str | None
    feedback_digest: str
    evidence_digest: str
    replan_requested: bool
    replan_instruction: str | None
    reward_input: Mapping[str, Any]
    evaluator_authority: str
    retention: str
    secret_material: str
    evaluation_digest: str

    def to_dict(self) -> dict[str, Any]:
        descriptor = {
            "schema": self.schema,
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "domain": self.domain,
            "workflow_id": self.workflow_id,
            "workflow_digest": self.workflow_digest,
            "contract_digest": self.contract_digest,
            "response_digest": self.response_digest,
            "signals": dict(self.signals),
            "missing_signals": list(self.missing_signals),
            "reward": self.reward,
            "passed": self.passed,
            "failed": self.failed,
            "failure_class": self.failure_class,
            "feedback_digest": self.feedback_digest,
            "evidence_digest": self.evidence_digest,
            "replan_requested": self.replan_requested,
            "replan_instruction": self.replan_instruction,
            "reward_input": dict(self.reward_input),
            "evaluator_authority": self.evaluator_authority,
            "retention": self.retention,
            "secret_material": self.secret_material,
        }
        descriptor["evaluation_digest"] = self.evaluation_digest
        return descriptor


def _stage_ids(profile: Any, workflow: Any | None = None) -> tuple[str, ...]:
    workflow = workflow if workflow is not None else getattr(profile, "workflow", None)
    stages = getattr(workflow, "stages", None)
    workflow_id = getattr(workflow, "workflow_id", "unknown")
    if not isinstance(stages, Sequence) or isinstance(stages, (str, bytes)) or not stages or len(stages) > MAX_AUTONOMOUS_DOMAIN_RESPONSE_ITEMS:
        raise ArgumentError(f"domain response workflow {workflow_id} has an invalid stage count")
    ids = tuple(_identifier("domain response stage id", getattr(stage, "id", None)) for stage in stages)
    if len(set(ids)) != len(ids):
        raise ArgumentError("domain response workflow stages contain duplicate ids")
    return ids


def _string_array_schema() -> dict[str, Any]:
    return {
        "type": "array",
        "items": {"type": "string", "maxLength": MAX_AUTONOMOUS_DOMAIN_RESPONSE_ITEM_BYTES},
        "maxItems": MAX_AUTONOMOUS_DOMAIN_RESPONSE_ITEMS,
    }


def _response_schema(domain: str, workflow_id: str, stage_ids: Sequence[str], fields: Sequence[str]) -> dict[str, Any]:
    stage_schema = {
        "type": "object",
        "additionalProperties": False,
        "properties": {
            "stage_id": {"type": "string", "enum": list(stage_ids)},
            "status": {"type": "string", "enum": list(AUTONOMOUS_DOMAIN_STAGE_RESPONSE_STATUSES)},
            "evidence": _string_array_schema(),
            "findings": _string_array_schema(),
            "uncertainty": _string_array_schema(),
            "open_questions": _string_array_schema(),
        },
        "required": ["stage_id", "status", "evidence", "findings", "uncertainty", "open_questions"],
    }
    return {
        "type": "object",
        "additionalProperties": False,
        "properties": {
            "schema": {"type": "string", "const": AUTONOMOUS_DOMAIN_RESPONSE_SCHEMA},
            "domain": {"type": "string", "const": domain},
            "workflow_id": {"type": "string", "const": workflow_id},
            "status": {"type": "string", "enum": list(AUTONOMOUS_DOMAIN_RESPONSE_STATUSES)},
            "answer": {"type": "string", "minLength": 1, "maxLength": MAX_AUTONOMOUS_DOMAIN_RESPONSE_ANSWER_BYTES},
            "observations": _string_array_schema(),
            "inferences": _string_array_schema(),
            "uncertainty": _string_array_schema(),
            "evidence_gaps": _string_array_schema(),
            "next_actions": _string_array_schema(),
            "stages": {"type": "array", "minItems": len(stage_ids), "maxItems": len(stage_ids), "items": stage_schema},
            "domain_details": {
                "type": "object",
                "additionalProperties": False,
                "properties": {field: _string_array_schema() for field in fields},
                "required": list(fields),
            },
            "retention": {"type": "string", "const": "transient_provider_response_only;validated_against_reviewed_domain_contract"},
            "secret_material": {"type": "string", "const": "never_returned"},
        },
        "required": ["schema", "domain", "workflow_id", "status", "answer", "observations", "inferences", "uncertainty", "evidence_gaps", "next_actions", "stages", "domain_details", "retention", "secret_material"],
    }


def _prompt_contract(domain: str, workflow_id: str, stage_ids: Sequence[str], fields: Sequence[str]) -> str:
    return " ".join(
        (
            f"Return only one JSON object matching the {AUTONOMOUS_DOMAIN_RESPONSE_SCHEMA} contract for domain {domain}.",
            f"Use workflow {workflow_id} and include exactly one stage row for every stage, in reviewed order: {', '.join(stage_ids)}.",
            f"Each stage must report status, evidence, findings, uncertainty, and open_questions. Populate every domain_details field: {', '.join(fields)}.",
            "Separate observations from inferences, mark missing evidence and uncertainty explicitly, and put proposed work in next_actions.",
            "Never claim that a provider response, tool dispatch, simulation, or plan proves an external-world effect.",
        )
    )


def build_autonomous_domain_response_contract(
    profile: Any,
    *,
    workflow: Any | None = None,
) -> AutonomousDomainResponseContract:
    """Build a digest-bound contract from a reviewed profile and workflow.

    Python keeps domain profiles and workflow strategies as separate registries, so callers must
    provide the resolved workflow when the profile does not carry one as an attribute.
    """

    domain = getattr(profile, "domain", None)
    workflow = workflow if workflow is not None else getattr(profile, "workflow", None)
    workflow_id = getattr(workflow, "workflow_id", None)
    workflow_digest = getattr(workflow, "workflow_digest", None)
    if not isinstance(domain, str) or workflow is None or not isinstance(workflow_id, str) or not isinstance(workflow_digest, str):
        raise ArgumentError("domain response contract requires a reviewed domain profile")
    workflow_domain = getattr(workflow, "domain", None)
    if workflow_domain is not None and workflow_domain != domain:
        raise ArgumentError("domain response workflow must align with the reviewed domain profile")
    stage_ids = _stage_ids(profile, workflow)
    fields = AUTONOMOUS_DOMAIN_RESPONSE_FIELDS.get(domain)
    if not fields:
        raise ArgumentError(f"domain response contract has no field set for {domain}")
    descriptor = {
        "schema": AUTONOMOUS_DOMAIN_RESPONSE_CONTRACT_SCHEMA,
        "version": "1",
        "domain": domain,
        "workflow_id": workflow_id,
        "workflow_digest": workflow_digest,
        "stage_ids": list(stage_ids),
        "domain_fields": list(fields),
        "response_schema": _response_schema(domain, workflow_id, stage_ids, fields),
        "prompt_contract": _prompt_contract(domain, workflow_id, stage_ids, fields),
        "retention": "contract_metadata_only;provider_response_remains_transient",
        "secret_material": "never_returned",
    }
    contract_digest = content_digest(descriptor)
    contract = AutonomousDomainResponseContract(contract_digest=contract_digest, **descriptor)
    if _json_bytes(contract.to_dict()) > MAX_AUTONOMOUS_DOMAIN_RESPONSE_CONTRACT_BYTES:
        raise ArgumentError("domain response contract exceeds its byte bound")
    return contract


def _contract_ids(contract: AutonomousDomainResponseContract) -> tuple[str, ...]:
    if not contract.stage_ids or len(set(contract.stage_ids)) != len(contract.stage_ids):
        raise ArgumentError("domain response contract stage_ids are malformed")
    return tuple(_identifier("domain response contract stage id", value) for value in contract.stage_ids)


def _contract_fields(contract: AutonomousDomainResponseContract) -> tuple[str, ...]:
    if not contract.domain_fields or len(set(contract.domain_fields)) != len(contract.domain_fields):
        raise ArgumentError("domain response contract domain_fields are malformed")
    fields = tuple(_text("domain response contract field", value, 256) for value in contract.domain_fields)
    if any(not _FIELD_IDENTIFIER.fullmatch(field) for field in fields):
        raise ArgumentError("domain response contract field is malformed")
    return fields


def validate_autonomous_domain_response(value: Any, contract: AutonomousDomainResponseContract) -> AutonomousDomainResponse:
    """Validate provider JSON beyond JSON Schema and normalize it to immutable values."""

    if not isinstance(contract, AutonomousDomainResponseContract):
        raise ArgumentError("domain response validation requires a contract")
    if not isinstance(value, Mapping):
        raise ArgumentError("domain response must be a JSON object")
    stage_ids = _contract_ids(contract)
    fields = _contract_fields(contract)
    _exact_keys(
        "domain response",
        value,
        ("schema", "domain", "workflow_id", "status", "answer", "observations", "inferences", "uncertainty", "evidence_gaps", "next_actions", "stages", "domain_details", "retention", "secret_material"),
    )
    if (
        value.get("schema") != AUTONOMOUS_DOMAIN_RESPONSE_SCHEMA
        or value.get("domain") != contract.domain
        or value.get("workflow_id") != contract.workflow_id
        or value.get("retention") != "transient_provider_response_only;validated_against_reviewed_domain_contract"
        or value.get("secret_material") != "never_returned"
    ):
        raise ArgumentError("domain response identity or retention markers are invalid")
    status = value.get("status")
    if status not in AUTONOMOUS_DOMAIN_RESPONSE_STATUSES:
        raise ArgumentError("domain response status is invalid")
    answer = _text("domain response answer", value.get("answer"), MAX_AUTONOMOUS_DOMAIN_RESPONSE_ANSWER_BYTES)
    observations = _bounded_list("domain response observations", value.get("observations"))
    inferences = _bounded_list("domain response inferences", value.get("inferences"))
    uncertainty = _bounded_list("domain response uncertainty", value.get("uncertainty"))
    evidence_gaps = _bounded_list("domain response evidence_gaps", value.get("evidence_gaps"))
    next_actions = _bounded_list("domain response next_actions", value.get("next_actions"))
    raw_stages = value.get("stages")
    if not isinstance(raw_stages, Sequence) or isinstance(raw_stages, (str, bytes, bytearray)) or len(raw_stages) != len(stage_ids):
        raise ArgumentError("domain response must contain exactly one row per reviewed stage")
    stages: list[AutonomousDomainStageResponse] = []
    for index, raw_stage in enumerate(raw_stages):
        if not isinstance(raw_stage, Mapping):
            raise ArgumentError("domain response stage row is malformed")
        _exact_keys("domain response stage", raw_stage, ("stage_id", "status", "evidence", "findings", "uncertainty", "open_questions"))
        if raw_stage.get("stage_id") != stage_ids[index]:
            raise ArgumentError("domain response stages must follow reviewed workflow order")
        stage_status = raw_stage.get("status")
        if stage_status not in AUTONOMOUS_DOMAIN_STAGE_RESPONSE_STATUSES:
            raise ArgumentError("domain response stage status is invalid")
        stages.append(
            AutonomousDomainStageResponse(
                stage_id=stage_ids[index],
                status=stage_status,
                evidence=_bounded_list("domain response stage evidence", raw_stage.get("evidence")),
                findings=_bounded_list("domain response stage findings", raw_stage.get("findings")),
                uncertainty=_bounded_list("domain response stage uncertainty", raw_stage.get("uncertainty")),
                open_questions=_bounded_list("domain response stage open_questions", raw_stage.get("open_questions")),
            )
        )
    raw_details = value.get("domain_details")
    if not isinstance(raw_details, Mapping):
        raise ArgumentError("domain response domain_details must be an object")
    _exact_keys("domain response domain_details", raw_details, fields)
    details = {field: _bounded_list(f"domain response domain_details.{field}", raw_details.get(field)) for field in fields}
    normalized = AutonomousDomainResponse(
        schema=AUTONOMOUS_DOMAIN_RESPONSE_SCHEMA,
        domain=contract.domain,
        workflow_id=contract.workflow_id,
        status=status,
        answer=answer,
        observations=observations,
        inferences=inferences,
        uncertainty=uncertainty,
        evidence_gaps=evidence_gaps,
        next_actions=next_actions,
        stages=tuple(stages),
        domain_details=details,
        retention="transient_provider_response_only;validated_against_reviewed_domain_contract",
        secret_material="never_returned",
    )
    _safe_value(normalized.to_dict())
    return normalized


def validate_autonomous_provider_domain_response(response: Any, contract: AutonomousDomainResponseContract | None) -> AutonomousDomainResponse | None:
    """Validate a provider response only when the caller selected structured domain mode."""

    if contract is None:
        return None
    structured = getattr(response, "structured", None) if response is not None else None
    if structured is None and isinstance(response, Mapping):
        structured = response.get("structured")
    if structured is None:
        raise ArgumentError("structured domain response is missing")
    return validate_autonomous_domain_response(structured, contract)


def evaluate_autonomous_domain_response(value: Any, contract: AutonomousDomainResponseContract) -> AutonomousDomainResponseEvaluation:
    """Return a deterministic structural reward for a validated response."""

    response = validate_autonomous_domain_response(value, contract)
    response_dict = response.to_dict()
    response_digest = content_digest(response_dict)
    stage_reporting = [int(bool(stage.evidence or stage.findings or stage.uncertainty or stage.open_questions)) for stage in response.stages]
    detail_reporting = [int(bool(response.domain_details[field])) for field in contract.domain_fields]
    signals: dict[str, float] = {
        "answer_present": float(bool(response.answer)),
        "stage_rows_complete": 1.0,
        "stage_reporting_coverage": _fraction(len(stage_reporting), sum(stage_reporting)),
        "domain_detail_coverage": _fraction(len(detail_reporting), sum(detail_reporting)),
        "observations_present": float(bool(response.observations)),
        "inferences_present": float(bool(response.inferences)),
        "uncertainty_disclosed": float(bool(response.uncertainty)),
        "evidence_gaps_disclosed": float(bool(response.evidence_gaps)),
        "next_actions_present": float(bool(response.next_actions)),
    }
    weights = {
        "answer_present": 1.0,
        "stage_rows_complete": 2.0,
        "stage_reporting_coverage": 2.0,
        "domain_detail_coverage": 2.0,
        "observations_present": 1.0,
        "inferences_present": 1.0,
        "uncertainty_disclosed": 1.5,
        "evidence_gaps_disclosed": 1.0,
        "next_actions_present": 1.0,
    }
    total_weight = sum(weights.values())
    reward = round(sum(signals[name] * weight for name, weight in weights.items()) / total_weight, 12)
    missing = tuple(name for name, score in signals.items() if score < 1.0)
    passed = reward >= AUTONOMOUS_DOMAIN_RESPONSE_PASS_THRESHOLD
    evaluator_id = f"autonomous-{contract.domain}-response-integrity"
    feedback_digest = content_digest({"contract_digest": contract.contract_digest, "response_digest": response_digest, "signals": signals})
    failure_class = None if passed else "response_integrity_gate"
    instruction = None if passed else f"Improve bounded {contract.domain} response composition: {', '.join(missing) or 'the response integrity score'}."
    reward_input = {
        "evaluator_id": evaluator_id,
        "evaluator_version": AUTONOMOUS_DOMAIN_RESPONSE_EVALUATOR_VERSION,
        "reward": reward,
        "passed": passed,
        "failed": not passed,
        "feedback_digest": feedback_digest,
        "failure_class": failure_class,
        "evidence_digest": response_digest,
    }
    descriptor = {
        "schema": AUTONOMOUS_DOMAIN_RESPONSE_EVALUATION_SCHEMA,
        "evaluator_id": evaluator_id,
        "evaluator_version": AUTONOMOUS_DOMAIN_RESPONSE_EVALUATOR_VERSION,
        "domain": contract.domain,
        "workflow_id": contract.workflow_id,
        "workflow_digest": contract.workflow_digest,
        "contract_digest": contract.contract_digest,
        "response_digest": response_digest,
        "signals": signals,
        "missing_signals": list(missing),
        "reward": reward,
        "passed": passed,
        "failed": not passed,
        "failure_class": failure_class,
        "feedback_digest": feedback_digest,
        "evidence_digest": response_digest,
        "replan_requested": not passed,
        "replan_instruction": instruction,
        "reward_input": reward_input,
        "evaluator_authority": "structural_response_contract_only;not_external_truth",
        "retention": "value_only;response_and_credentials_not_retained",
        "secret_material": "never_returned",
    }
    evaluation_digest = content_digest(descriptor)
    return AutonomousDomainResponseEvaluation(evaluation_digest=evaluation_digest, **descriptor)


def validate_autonomous_domain_response_evaluation(value: Any) -> AutonomousDomainResponseEvaluation:
    """Validate a persisted structural response-evaluation projection.

    A response evaluation is intentionally portable without the provider response or reviewed
    workflow.  This validator therefore checks the evaluator identity, bounded signal range,
    digest bindings, and the canonical evaluation digest before a caller can use the projection
    as delayed learning credit.  It does not make the structural score evidence of task
    correctness or of an external-world effect.
    """

    if not isinstance(value, Mapping):
        raise ArgumentError("domain response evaluation must be a mapping")
    _safe_value(value)
    allowed = {
        "schema", "evaluator_id", "evaluator_version", "domain", "workflow_id", "workflow_digest",
        "contract_digest", "response_digest", "signals", "missing_signals", "reward", "passed",
        "failed", "failure_class", "feedback_digest", "evidence_digest", "replan_requested",
        "replan_instruction", "reward_input", "evaluator_authority", "retention", "secret_material",
        "evaluation_digest",
    }
    if set(value) != allowed:
        raise ArgumentError("domain response evaluation contains unsupported or missing fields")
    if value.get("schema") != AUTONOMOUS_DOMAIN_RESPONSE_EVALUATION_SCHEMA:
        raise ArgumentError("domain response evaluation schema is invalid")
    evaluator_id = _identifier("domain response evaluator_id", value.get("evaluator_id"))
    evaluator_version = _identifier("domain response evaluator_version", value.get("evaluator_version"))
    domain = _identifier("domain response evaluation domain", value.get("domain"))
    workflow_id = _identifier("domain response evaluation workflow_id", value.get("workflow_id"))
    for field in ("workflow_digest", "contract_digest", "response_digest", "feedback_digest", "evidence_digest", "evaluation_digest"):
        digest = value.get(field)
        if not isinstance(digest, str) or not re.fullmatch(r"[0-9a-f]{64}", digest):
            raise ArgumentError(f"domain response evaluation {field} must be a lowercase SHA-256 digest")
    if value.get("evaluator_authority") != "structural_response_contract_only;not_external_truth":
        raise ArgumentError("domain response evaluation authority marker is invalid")
    if value.get("retention") != "value_only;response_and_credentials_not_retained":
        raise ArgumentError("domain response evaluation retention marker is invalid")
    if value.get("secret_material") != "never_returned":
        raise ArgumentError("domain response evaluation secret marker is invalid")

    raw_signals = value.get("signals")
    if not isinstance(raw_signals, Mapping) or not raw_signals:
        raise ArgumentError("domain response evaluation signals must be a non-empty mapping")
    signals: dict[str, float] = {}
    for key, raw_score in raw_signals.items():
        signal = _identifier("domain response evaluation signal", key)
        if isinstance(raw_score, bool) or not isinstance(raw_score, (int, float)) or not math.isfinite(float(raw_score)) or not 0.0 <= float(raw_score) <= 1.0:
            raise ArgumentError("domain response evaluation signal scores must be finite values within [0, 1]")
        signals[signal] = float(raw_score)
    raw_missing = value.get("missing_signals")
    if not isinstance(raw_missing, Sequence) or isinstance(raw_missing, (str, bytes, bytearray)):
        raise ArgumentError("domain response evaluation missing_signals must be a sequence")
    missing_signals = tuple(_identifier("domain response missing signal", item) for item in raw_missing)
    if len(set(missing_signals)) != len(missing_signals) or any(signal not in signals for signal in missing_signals):
        raise ArgumentError("domain response evaluation missing_signals are inconsistent with signals")

    reward = value.get("reward")
    if isinstance(reward, bool) or not isinstance(reward, (int, float)) or not math.isfinite(float(reward)) or not 0.0 <= float(reward) <= 1.0:
        raise ArgumentError("domain response evaluation reward must be finite and within [0, 1]")
    passed = value.get("passed")
    failed = value.get("failed")
    replan_requested = value.get("replan_requested")
    if not isinstance(passed, bool) or not isinstance(failed, bool) or failed == passed:
        raise ArgumentError("domain response evaluation passed and failed flags are inconsistent")
    if not isinstance(replan_requested, bool) or replan_requested != failed:
        raise ArgumentError("domain response evaluation replan_requested is inconsistent")
    failure_class = value.get("failure_class")
    if failure_class is not None:
        failure_class = _identifier("domain response failure_class", failure_class)
    elif not passed:
        raise ArgumentError("failed domain response evaluations require a failure_class")
    instruction = value.get("replan_instruction")
    if instruction is not None:
        instruction = _text("domain response replan_instruction", instruction, MAX_AUTONOMOUS_DOMAIN_RESPONSE_ITEM_BYTES)
    elif not passed:
        raise ArgumentError("failed domain response evaluations require a replan_instruction")

    raw_reward_input = value.get("reward_input")
    if not isinstance(raw_reward_input, Mapping) or set(raw_reward_input) != {
        "evaluator_id", "evaluator_version", "reward", "passed", "failed", "feedback_digest", "failure_class", "evidence_digest",
    }:
        raise ArgumentError("domain response evaluation reward_input is malformed")
    if (
        raw_reward_input.get("evaluator_id") != evaluator_id
        or raw_reward_input.get("evaluator_version") != evaluator_version
        or raw_reward_input.get("reward") != reward
        or raw_reward_input.get("passed") != passed
        or raw_reward_input.get("failed") != failed
        or raw_reward_input.get("feedback_digest") != value.get("feedback_digest")
        or raw_reward_input.get("failure_class") != failure_class
        or raw_reward_input.get("evidence_digest") != value.get("evidence_digest")
    ):
        raise ArgumentError("domain response evaluation reward_input does not match its projection")

    descriptor = dict(value)
    descriptor.pop("evaluation_digest")
    expected_digest = content_digest(descriptor)
    if value.get("evaluation_digest") != expected_digest:
        raise ArgumentError("domain response evaluation digest does not match its projection")
    return AutonomousDomainResponseEvaluation(
        schema=AUTONOMOUS_DOMAIN_RESPONSE_EVALUATION_SCHEMA,
        evaluator_id=evaluator_id,
        evaluator_version=evaluator_version,
        domain=domain,
        workflow_id=workflow_id,
        workflow_digest=value["workflow_digest"],
        contract_digest=value["contract_digest"],
        response_digest=value["response_digest"],
        signals=signals,
        missing_signals=missing_signals,
        reward=float(reward),
        passed=passed,
        failed=failed,
        failure_class=failure_class,
        feedback_digest=value["feedback_digest"],
        evidence_digest=value["evidence_digest"],
        replan_requested=replan_requested,
        replan_instruction=instruction,
        reward_input=dict(raw_reward_input),
        evaluator_authority=value["evaluator_authority"],
        retention=value["retention"],
        secret_material=value["secret_material"],
        evaluation_digest=value["evaluation_digest"],
    )


def replay_autonomous_domain_response_evaluation(
    value: Any,
    contract: AutonomousDomainResponseContract,
    expected: AutonomousDomainResponseEvaluation | Mapping[str, Any],
) -> AutonomousDomainResponseEvaluation:
    """Re-run the structural evaluator and reject replay drift."""

    expected_digest = expected.evaluation_digest if isinstance(expected, AutonomousDomainResponseEvaluation) else expected.get("evaluation_digest") if isinstance(expected, Mapping) else None
    if not isinstance(expected_digest, str):
        raise ArgumentError("domain response replay requires an evaluation digest")
    replayed = evaluate_autonomous_domain_response(value, contract)
    if replayed.evaluation_digest != expected_digest:
        raise ArgumentError("domain response evaluator replay drifted from the recorded evaluation")
    return replayed


__all__ = [
    "AUTONOMOUS_DOMAIN_RESPONSE_SCHEMA",
    "AUTONOMOUS_DOMAIN_RESPONSE_CONTRACT_SCHEMA",
    "AUTONOMOUS_DOMAIN_RESPONSE_EVALUATION_SCHEMA",
    "AUTONOMOUS_DOMAIN_RESPONSE_STATUSES",
    "AUTONOMOUS_DOMAIN_STAGE_RESPONSE_STATUSES",
    "MAX_AUTONOMOUS_DOMAIN_RESPONSE_ITEMS",
    "MAX_AUTONOMOUS_DOMAIN_RESPONSE_ITEM_BYTES",
    "MAX_AUTONOMOUS_DOMAIN_RESPONSE_ANSWER_BYTES",
    "MAX_AUTONOMOUS_DOMAIN_RESPONSE_CONTRACT_BYTES",
    "AUTONOMOUS_DOMAIN_RESPONSE_EVALUATOR_VERSION",
    "AUTONOMOUS_DOMAIN_RESPONSE_PASS_THRESHOLD",
    "AUTONOMOUS_DOMAIN_RESPONSE_FIELDS",
    "AutonomousDomainStageResponse",
    "AutonomousDomainResponse",
    "AutonomousDomainResponseContract",
    "AutonomousDomainResponseEvaluation",
    "build_autonomous_domain_response_contract",
    "validate_autonomous_domain_response",
    "validate_autonomous_provider_domain_response",
    "evaluate_autonomous_domain_response",
    "validate_autonomous_domain_response_evaluation",
    "replay_autonomous_domain_response_evaluation",
]
