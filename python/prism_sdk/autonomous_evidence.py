"""Evidence-first planning primitives for the autonomous brain.

Workflow stage definitions already describe the evidence a useful answer needs, but callers
previously had to inspect those definitions themselves and reconstruct coverage logic.  This
module turns the reviewed stage catalogue into a bounded, digest-bound evidence contract.

The planner is deliberately provider- and connector-free.  It does not retrieve anything, score
truth, or grant authority.  It only identifies the evidence outputs required by reviewed stages,
matches caller-supplied output labels, and reports the next dependency-safe stages.  Raw evidence
and source payloads remain transient and caller-owned.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .authoring import content_digest
from .errors import ArgumentError


AUTONOMOUS_EVIDENCE_PLAN_SCHEMA = "bioprism-python-autonomous-evidence-plan/0.1"
AUTONOMOUS_EVIDENCE_REQUIREMENT_SCHEMA = "bioprism-python-autonomous-evidence-requirement/0.1"
AUTONOMOUS_EVIDENCE_COVERAGE_STATUSES = ("not_evaluated", "missing", "partial", "complete")
MAX_AUTONOMOUS_EVIDENCE_WORKFLOWS = 16
MAX_AUTONOMOUS_EVIDENCE_REQUIREMENTS = 512
MAX_AUTONOMOUS_EVIDENCE_LABEL_BYTES = 256
MAX_AUTONOMOUS_EVIDENCE_PLAN_BYTES = 256_000


def _bounded_identifier(name: str, value: Any) -> str:
    if not isinstance(value, str) or not value.strip() or len(value.encode("utf-8")) > MAX_AUTONOMOUS_EVIDENCE_LABEL_BYTES:
        raise ArgumentError(f"{name} must be a bounded non-empty identifier")
    if any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:-+ /" for character in value):
        raise ArgumentError(f"{name} contains unsupported characters")
    return value.strip()


def _bounded_text(name: str, value: Any) -> str:
    if not isinstance(value, str) or not value.strip() or len(value.encode("utf-8")) > 2_048:
        raise ArgumentError(f"{name} must be bounded non-empty text")
    if "\x00" in value:
        raise ArgumentError(f"{name} contains a NUL character")
    return value.strip()


def _bounded_list(name: str, value: Any, *, maximum: int) -> tuple[str, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be a sequence")
    if len(value) > maximum:
        raise ArgumentError(f"{name} exceeds its bound")
    result: list[str] = []
    seen: set[str] = set()
    for item in value:
        normalized = _bounded_identifier(f"{name} entry", item)
        if normalized in seen:
            raise ArgumentError(f"{name} contains a duplicate entry: {normalized}")
        seen.add(normalized)
        result.append(normalized)
    return tuple(result)


def _workflow_field(workflow: Any, field_name: str, default: Any = None) -> Any:
    if isinstance(workflow, Mapping):
        return workflow.get(field_name, default)
    return getattr(workflow, field_name, default)


def _stage_field(stage: Any, field_name: str, default: Any = None) -> Any:
    if isinstance(stage, Mapping):
        return stage.get(field_name, default)
    return getattr(stage, field_name, default)


def _validate_stage_graph(*, domain: str, stages: Sequence[Any]) -> tuple[str, ...]:
    """Validate reviewed stage edges before projecting readiness."""

    stage_ids: list[str] = []
    dependencies_by_stage: dict[str, tuple[str, ...]] = {}
    for stage in stages:
        stage_id = _bounded_identifier("autonomous evidence stage id", _stage_field(stage, "id"))
        if stage_id in dependencies_by_stage:
            raise ArgumentError("autonomous evidence workflow stage IDs must be unique")
        dependencies = _bounded_list(
            "autonomous evidence stage dependencies",
            _stage_field(stage, "depends_on", ()),
            maximum=64,
        )
        if stage_id in dependencies:
            raise ArgumentError(f"autonomous evidence stage {domain}:{stage_id} cannot depend on itself")
        stage_ids.append(stage_id)
        dependencies_by_stage[stage_id] = dependencies
    known = set(stage_ids)
    unknown = sorted({dependency for values in dependencies_by_stage.values() for dependency in values if dependency not in known})
    if unknown:
        raise ArgumentError(
            f"autonomous evidence workflow {domain} has unknown stage dependencies: {', '.join(unknown)}"
        )
    indegree = {stage_id: len(values) for stage_id, values in dependencies_by_stage.items()}
    dependents: dict[str, list[str]] = {stage_id: [] for stage_id in stage_ids}
    for stage_id, values in dependencies_by_stage.items():
        for dependency in values:
            dependents[dependency].append(stage_id)
    ready = [stage_id for stage_id in stage_ids if indegree[stage_id] == 0]
    visited = 0
    while ready:
        current = ready.pop(0)
        visited += 1
        for dependent in dependents[current]:
            indegree[dependent] -= 1
            if indegree[dependent] == 0:
                ready.append(dependent)
    if visited != len(stage_ids):
        raise ArgumentError(f"autonomous evidence workflow {domain} contains a dependency cycle")
    return tuple(stage_ids)


@dataclass(frozen=True, slots=True)
class AutonomousEvidenceRequirement:
    """One evidence output required by a reviewed workflow stage."""

    requirement_id: str
    domain: str
    workflow_id: str
    workflow_digest: str
    stage_id: str
    label: str
    objective: str
    required_capabilities: tuple[str, ...]
    evaluator_signals: tuple[str, ...]
    depends_on: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        for name, value in (
            ("requirement_id", self.requirement_id),
            ("domain", self.domain),
            ("workflow_id", self.workflow_id),
            ("stage_id", self.stage_id),
            ("label", self.label),
        ):
            _bounded_identifier(f"evidence requirement {name}", value)
        _bounded_text("evidence requirement objective", self.objective)
        if not isinstance(self.workflow_digest, str) or len(self.workflow_digest) != 64 or any(
            character not in "0123456789abcdef" for character in self.workflow_digest
        ):
            raise ArgumentError("evidence requirement workflow_digest must be a lowercase SHA-256 digest")
        capabilities = _bounded_list(
            "evidence requirement required_capabilities", self.required_capabilities, maximum=64
        )
        signals = _bounded_list(
            "evidence requirement evaluator_signals", self.evaluator_signals, maximum=64
        )
        dependencies = _bounded_list("evidence requirement depends_on", self.depends_on, maximum=64)
        if not capabilities:
            raise ArgumentError("evidence requirement must retain at least one required capability")
        object.__setattr__(self, "required_capabilities", capabilities)
        object.__setattr__(self, "evaluator_signals", signals)
        object.__setattr__(self, "depends_on", dependencies)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_REQUIREMENT_SCHEMA,
            "requirement_id": self.requirement_id,
            "domain": self.domain,
            "workflow_id": self.workflow_id,
            "workflow_digest": self.workflow_digest,
            "stage_id": self.stage_id,
            "label": self.label,
            "objective": self.objective,
            "required_capabilities": list(self.required_capabilities),
            "evaluator_signals": list(self.evaluator_signals),
            "depends_on": list(self.depends_on),
        }


@dataclass(frozen=True, slots=True)
class AutonomousEvidencePlan:
    """A value-only evidence contract and coverage projection for one or more workflows."""

    domains: tuple[str, ...]
    workflow_ids: tuple[str, ...]
    workflow_digests: tuple[str, ...]
    requirements: tuple[AutonomousEvidenceRequirement, ...]
    available_evidence: tuple[str, ...] = ()
    covered_requirement_ids: tuple[str, ...] = ()
    missing_requirement_ids: tuple[str, ...] = ()
    next_stage_ids: tuple[str, ...] = ()
    coverage_status: str = "not_evaluated"

    def __post_init__(self) -> None:
        domains = _bounded_list("evidence plan domains", self.domains, maximum=MAX_AUTONOMOUS_EVIDENCE_WORKFLOWS)
        workflows = _bounded_list("evidence plan workflow_ids", self.workflow_ids, maximum=MAX_AUTONOMOUS_EVIDENCE_WORKFLOWS)
        digests = tuple(self.workflow_digests)
        if len(digests) != len(workflows) or any(
            not isinstance(digest, str)
            or len(digest) != 64
            or any(character not in "0123456789abcdef" for character in digest)
            for digest in digests
        ):
            raise ArgumentError("evidence plan workflow digests must align with workflows")
        if len(domains) != len(workflows):
            raise ArgumentError("evidence plan domains must align with workflows")
        if not isinstance(self.requirements, Sequence) or isinstance(self.requirements, (str, bytes)):
            raise ArgumentError("evidence plan requirements must be a sequence")
        requirements = tuple(self.requirements)
        if len(requirements) > MAX_AUTONOMOUS_EVIDENCE_REQUIREMENTS or any(
            not isinstance(item, AutonomousEvidenceRequirement) for item in requirements
        ):
            raise ArgumentError("evidence plan requirements are outside their bound")
        ids = tuple(item.requirement_id for item in requirements)
        if len(set(ids)) != len(ids):
            raise ArgumentError("evidence plan requirement IDs must be unique")
        workflow_by_id = dict(zip(workflows, domains))
        digest_by_workflow = dict(zip(workflows, digests))
        for item in requirements:
            if item.workflow_id not in workflow_by_id:
                raise ArgumentError("evidence plan requirement references an unknown workflow")
            if item.domain != workflow_by_id[item.workflow_id] or item.workflow_digest != digest_by_workflow[item.workflow_id]:
                raise ArgumentError("evidence plan requirement workflow identity is inconsistent")
        available = _bounded_list("evidence plan available_evidence", self.available_evidence, maximum=MAX_AUTONOMOUS_EVIDENCE_REQUIREMENTS)
        covered = _bounded_list("evidence plan covered_requirement_ids", self.covered_requirement_ids, maximum=MAX_AUTONOMOUS_EVIDENCE_REQUIREMENTS)
        missing = _bounded_list("evidence plan missing_requirement_ids", self.missing_requirement_ids, maximum=MAX_AUTONOMOUS_EVIDENCE_REQUIREMENTS)
        next_stages = _bounded_list("evidence plan next_stage_ids", self.next_stage_ids, maximum=MAX_AUTONOMOUS_EVIDENCE_REQUIREMENTS)
        if set(covered).intersection(missing) or set(covered).union(missing) != set(ids):
            raise ArgumentError("evidence plan covered and missing IDs must partition requirements")
        if any(value not in ids for value in covered + missing):
            raise ArgumentError("evidence plan references an unknown requirement")
        if self.coverage_status not in AUTONOMOUS_EVIDENCE_COVERAGE_STATUSES:
            raise ArgumentError("evidence plan coverage_status is invalid")
        payload = self._payload(
            domains=domains,
            workflow_ids=workflows,
            workflow_digests=digests,
            requirements=requirements,
            available=available,
            covered=covered,
            missing=missing,
            next_stages=next_stages,
        )
        import json

        if len(json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")) > MAX_AUTONOMOUS_EVIDENCE_PLAN_BYTES:
            raise ArgumentError("evidence plan exceeds its bounded size")
        object.__setattr__(self, "domains", domains)
        object.__setattr__(self, "workflow_ids", workflows)
        object.__setattr__(self, "workflow_digests", digests)
        object.__setattr__(self, "requirements", requirements)
        object.__setattr__(self, "available_evidence", available)
        object.__setattr__(self, "covered_requirement_ids", covered)
        object.__setattr__(self, "missing_requirement_ids", missing)
        object.__setattr__(self, "next_stage_ids", next_stages)

    def _payload(self, **values: Any) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_EVIDENCE_PLAN_SCHEMA,
            "domains": list(values.get("domains", self.domains)),
            "workflow_ids": list(values.get("workflow_ids", self.workflow_ids)),
            "workflow_digests": list(values.get("workflow_digests", self.workflow_digests)),
            "requirements": [item.to_dict() for item in values.get("requirements", self.requirements)],
            "available_evidence": list(values.get("available", self.available_evidence)),
            "covered_requirement_ids": list(values.get("covered", self.covered_requirement_ids)),
            "missing_requirement_ids": list(values.get("missing", self.missing_requirement_ids)),
            "next_stage_ids": list(values.get("next_stages", self.next_stage_ids)),
            "coverage_status": self.coverage_status,
        }

    @property
    def plan_digest(self) -> str:
        return content_digest(self._payload())

    @property
    def coverage_ratio(self) -> float:
        if not self.requirements:
            return 1.0
        return len(self.covered_requirement_ids) / len(self.requirements)

    def to_dict(self) -> dict[str, Any]:
        return {
            **self._payload(),
            "plan_digest": self.plan_digest,
            "coverage_ratio": self.coverage_ratio,
            "retention": "evidence_contract_and_digests_only;raw_payloads_caller_owned",
            "execution": "planning_only;no_source_or_provider_dispatch",
            "does_not_claim": [
                "evidence was acquired",
                "a source is truthful or current",
                "a connector, tool, provider, or credential is available",
                "coverage proves task completion",
            ],
            "secret_material": "never_returned",
        }

    def with_available_evidence(self, available_evidence: Sequence[str]) -> "AutonomousEvidencePlan":
        """Recompute coverage after a caller-owned adapter produces new evidence identifiers."""

        available = _bounded_list(
            "evidence plan available_evidence",
            available_evidence,
            maximum=MAX_AUTONOMOUS_EVIDENCE_REQUIREMENTS,
        )
        by_label: dict[str, list[str]] = {}
        for item in self.requirements:
            by_label.setdefault(item.label, []).append(item.requirement_id)
        covered = tuple(
            item.requirement_id
            for item in self.requirements
            if item.requirement_id in available
            or (item.label in available and len(by_label.get(item.label, ())) == 1)
        )
        missing = tuple(item.requirement_id for item in self.requirements if item.requirement_id not in covered)
        status = (
            "not_evaluated"
            if not available
            else "complete"
            if not missing
            else "partial"
            if covered
            else "missing"
        )
        return AutonomousEvidencePlan(
            domains=self.domains,
            workflow_ids=self.workflow_ids,
            workflow_digests=self.workflow_digests,
            requirements=self.requirements,
            available_evidence=available,
            covered_requirement_ids=covered,
            missing_requirement_ids=missing,
            next_stage_ids=self.next_stage_ids,
            coverage_status=status,
        )


def build_autonomous_evidence_plan(
    workflows: Sequence[Any],
    *,
    available_evidence: Sequence[str] = (),
    completed_stages: Mapping[str, Sequence[str]] | None = None,
) -> AutonomousEvidencePlan:
    """Compile reviewed workflows into an evidence plan without executing anything."""

    if not isinstance(workflows, Sequence) or isinstance(workflows, (str, bytes)):
        raise ArgumentError("autonomous evidence workflows must be a sequence")
    if not 1 <= len(workflows) <= MAX_AUTONOMOUS_EVIDENCE_WORKFLOWS:
        raise ArgumentError("autonomous evidence workflows must contain 1..16 workflows")
    completed = {} if completed_stages is None else dict(completed_stages)
    if any(not isinstance(domain, str) or not isinstance(stages, Sequence) or isinstance(stages, (str, bytes)) for domain, stages in completed.items()):
        raise ArgumentError("autonomous evidence completed_stages must map domains to sequences")
    requirements: list[AutonomousEvidenceRequirement] = []
    domains: list[str] = []
    workflow_ids: list[str] = []
    workflow_digests: list[str] = []
    for workflow in workflows:
        domain = _bounded_identifier("autonomous evidence workflow domain", _workflow_field(workflow, "domain"))
        workflow_id = _bounded_identifier("autonomous evidence workflow id", _workflow_field(workflow, "workflow_id"))
        workflow_digest = _workflow_field(workflow, "workflow_digest")
        if not isinstance(workflow_digest, str) or len(workflow_digest) != 64 or any(character not in "0123456789abcdef" for character in workflow_digest):
            raise ArgumentError("autonomous evidence workflow digest must be a lowercase SHA-256 digest")
        if domain in domains or workflow_id in workflow_ids:
            raise ArgumentError("autonomous evidence workflows must have unique domains and workflow IDs")
        stages = _workflow_field(workflow, "stages")
        if not isinstance(stages, Sequence) or isinstance(stages, (str, bytes)) or not stages:
            raise ArgumentError("autonomous evidence workflow must contain stages")
        domains.append(domain)
        workflow_ids.append(workflow_id)
        workflow_digests.append(workflow_digest)
        stage_ids = _validate_stage_graph(domain=domain, stages=stages)
        for stage in stages:
            stage_id = _bounded_identifier("autonomous evidence stage id", _stage_field(stage, "id"))
            outputs = _bounded_list("autonomous evidence stage outputs", _stage_field(stage, "evidence_outputs", ()), maximum=64)
            capabilities = _bounded_list("autonomous evidence stage capabilities", _stage_field(stage, "required_capabilities", ()), maximum=64)
            signals = _bounded_list("autonomous evidence stage evaluator signals", _stage_field(stage, "evaluator_signals", ()), maximum=64)
            dependencies = _bounded_list("autonomous evidence stage dependencies", _stage_field(stage, "depends_on", ()), maximum=64)
            objective = _bounded_text("autonomous evidence stage objective", _stage_field(stage, "objective"))
            if not capabilities:
                raise ArgumentError("autonomous evidence stage must require a capability")
            for output in outputs:
                requirement_id = f"{domain}:{stage_id}:{output}"
                requirements.append(
                    AutonomousEvidenceRequirement(
                        requirement_id=requirement_id,
                        domain=domain,
                        workflow_id=workflow_id,
                        workflow_digest=workflow_digest,
                        stage_id=stage_id,
                        label=output,
                        objective=objective,
                        required_capabilities=capabilities,
                        evaluator_signals=signals,
                        depends_on=dependencies,
                    )
                )
    if len(requirements) > MAX_AUTONOMOUS_EVIDENCE_REQUIREMENTS:
        raise ArgumentError("autonomous evidence requirements exceed their bound")
    available = _bounded_list("autonomous evidence available_evidence", available_evidence, maximum=MAX_AUTONOMOUS_EVIDENCE_REQUIREMENTS)
    requirement_ids = {item.requirement_id for item in requirements}
    by_label: dict[str, list[str]] = {}
    for item in requirements:
        by_label.setdefault(item.label, []).append(item.requirement_id)
    covered: list[str] = []
    for item in requirements:
        if item.requirement_id in available or (item.label in available and len(by_label[item.label]) == 1):
            covered.append(item.requirement_id)
    missing = [item.requirement_id for item in requirements if item.requirement_id not in covered]
    if any(item not in requirement_ids and item not in by_label for item in available):
        # Unknown labels are accepted as caller-owned observations; they simply do not satisfy a
        # reviewed output.  This keeps the planner useful for richer external evidence catalogs.
        pass
    completed_by_domain: dict[str, set[str]] = {
        domain: set(_bounded_list(f"completed stages for {domain}", stages, maximum=64))
        for domain, stages in completed.items()
    }
    next_stages: list[str] = []
    for workflow in workflows:
        domain = _bounded_identifier("autonomous evidence workflow domain", _workflow_field(workflow, "domain"))
        stages = _workflow_field(workflow, "stages")
        done = completed_by_domain.get(domain, set())
        known_stage_ids = {
            _bounded_identifier("autonomous evidence stage id", _stage_field(stage, "id"))
            for stage in stages
        }
        unknown_completed = sorted(done.difference(known_stage_ids))
        if unknown_completed:
            raise ArgumentError(
                f"completed stages for {domain} reference unknown stages: {', '.join(unknown_completed)}"
            )
        for stage in stages:
            stage_id = _bounded_identifier("autonomous evidence stage id", _stage_field(stage, "id"))
            dependencies = set(_bounded_list("autonomous evidence stage dependencies", _stage_field(stage, "depends_on", ()), maximum=64))
            if stage_id not in done and dependencies.issubset(done):
                next_stages.append(f"{domain}:{stage_id}")
    if not available:
        status = "not_evaluated"
    elif not missing:
        status = "complete"
    elif covered:
        status = "partial"
    else:
        status = "missing"
    return AutonomousEvidencePlan(
        domains=tuple(domains),
        workflow_ids=tuple(workflow_ids),
        workflow_digests=tuple(workflow_digests),
        requirements=tuple(requirements),
        available_evidence=available,
        covered_requirement_ids=tuple(covered),
        missing_requirement_ids=tuple(missing),
        next_stage_ids=tuple(next_stages),
        coverage_status=status,
    )
