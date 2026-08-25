"""Evaluator-gated consolidation for autonomous episodic memory.

Episodic memory answers *what happened before*.  Consolidation answers a narrower and safer
question: which caller-authored lessons have enough independent evaluator-backed support to be
shown as a reusable reference for a future run?  This module deliberately stores lesson and
evidence digests rather than lesson text, prompts, tasks, provider output, credentials, or tool
arguments.  A caller may resolve a digest to transient prompt text only after the returned row has
passed the same domain/scope policy.

The consolidation boundary is provider-free and domain-neutral.  It uses explicit evaluator
rewards and evidence digests, keeps portable lessons separate from domain-local lessons, marks
competing variants as conflicts, and persists a canonical digest-bound snapshot with optional
compare-and-swap fencing.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
import math
import re
import time
from typing import Any, Callable, Mapping, Protocol, Sequence

from .authoring import canonical_json, content_digest
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES


AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEMA = "bioprism-python-autonomous-memory-consolidation/0.1"
AUTONOMOUS_MEMORY_CONSOLIDATION_LESSON_SCHEMA = "bioprism-python-autonomous-memory-consolidation-lesson/0.1"
AUTONOMOUS_MEMORY_CONSOLIDATION_SNAPSHOT_SCHEMA = "bioprism-python-autonomous-memory-consolidation-snapshot/0.1"
MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_OBSERVATIONS = 16_384
MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_LESSONS = 4_096
MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_DOMAINS = len(AUTONOMOUS_DOMAIN_NAMES)
MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_ID_BYTES = 256
MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SNAPSHOT_BYTES = 8_000_000
MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_PROMPT_LESSONS = 32

_DOMAINS = tuple(AUTONOMOUS_DOMAIN_NAMES)
_ID_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.:-]{0,255}$")
_STATUSES = ("candidate", "stable", "conflicted", "stale")
_SCOPES = ("domain", "cross_domain")
_RETENTION = "metadata_only_lesson_evidence_and_episode_digests_no_text_or_payloads"
_SECRET_MATERIAL = "never_returned"


class AutonomousMemoryConsolidationError(ValueError):
    """Raised when consolidation input, replay, or persistence is unsafe."""


def _fail(message: str) -> None:
    raise AutonomousMemoryConsolidationError(f"memory consolidation {message}")


def _identifier(name: str, value: Any) -> str:
    if not isinstance(value, str) or not value or len(value.encode("utf-8")) > MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_ID_BYTES or not _ID_RE.fullmatch(value):
        _fail(f"{name} is not a bounded identifier")
    return value


def _digest(name: str, value: Any, *, optional: bool = False) -> str | None:
    if optional and value is None:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        _fail(f"{name} must be a lowercase SHA-256 digest")
    return value


def _domain(value: Any) -> str:
    if not isinstance(value, str) or value not in _DOMAINS:
        _fail("domain is not a supported built-in autonomous domain")
    return value


def _bounded_number(name: str, value: Any, minimum: float, maximum: float) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)) or not minimum <= float(value) <= maximum:
        _fail(f"{name} is outside its numeric bounds")
    return float(value)


def _bounded_integer(name: str, value: Any, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        _fail(f"{name} is outside its integer bounds")
    return value


def _boolean(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        _fail(f"{name} must be boolean")
    return value


def _string_tuple(name: str, value: Any, *, maximum: int = 64) -> tuple[str, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)) or len(value) > maximum:
        _fail(f"{name} must be a bounded sequence")
    result: list[str] = []
    for item in value:
        normalized = _identifier(f"{name} item", item)
        if normalized in result:
            _fail(f"{name} contains duplicates")
        result.append(normalized)
    return tuple(sorted(result))


def _wilson_lower(successes: int, observations: int) -> float:
    if observations <= 0:
        return 0.0
    z = 1.959963984540054
    rate = successes / observations
    denominator = 1 + z * z / observations
    center = rate + z * z / (2 * observations)
    spread = z * math.sqrt((rate * (1 - rate) + z * z / (4 * observations)) / observations)
    return max(0.0, min(1.0, (center - spread) / denominator))


def _normalized_reward(reward: float) -> float:
    # Reward inputs may use the shared [-1, 1] bandit range; the report exposes [0, 1]
    # quality metrics so the TypeScript and Python projections have the same meaning.
    return (reward + 1.0) / 2.0


@dataclass(frozen=True, slots=True)
class AutonomousMemoryConsolidationObservation:
    """One explicit evaluator-backed lesson observation."""

    episode_id: str
    lesson_id: str
    concept_id: str
    variant_id: str
    domain: str
    capability: str
    risk_class: str
    evaluator_id: str
    evaluator_version: str
    reward: float
    passed: bool
    evidence_digest: str
    lesson_digest: str
    decision_digest: str | None = None
    observed_at: float = 0.0
    transferable: bool = False

    def __post_init__(self) -> None:
        for name in ("episode_id", "lesson_id", "concept_id", "variant_id", "capability", "risk_class", "evaluator_id", "evaluator_version"):
            object.__setattr__(self, name, _identifier(f"observation {name}", getattr(self, name)))
        object.__setattr__(self, "domain", _domain(self.domain))
        object.__setattr__(self, "reward", _bounded_number("observation reward", self.reward, -1.0, 1.0))
        object.__setattr__(self, "passed", _boolean("observation passed", self.passed))
        object.__setattr__(self, "evidence_digest", _digest("observation evidence_digest", self.evidence_digest))
        object.__setattr__(self, "lesson_digest", _digest("observation lesson_digest", self.lesson_digest))
        object.__setattr__(self, "decision_digest", _digest("observation decision_digest", self.decision_digest, optional=True))
        object.__setattr__(self, "observed_at", _bounded_number("observation observed_at", self.observed_at, 0.0, 9_223_372_036_854_775.0))
        object.__setattr__(self, "transferable", _boolean("observation transferable", self.transferable))

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousMemoryConsolidationObservation":
        if not isinstance(value, Mapping):
            _fail("observation must be an object")
        allowed = {
            "episode_id", "lesson_id", "concept_id", "variant_id", "domain", "capability", "risk_class",
            "evaluator_id", "evaluator_version", "reward", "passed", "evidence_digest", "lesson_digest",
            "decision_digest", "observed_at", "transferable",
        }
        if set(value).difference(allowed):
            _fail("observation contains unsupported fields")
        return cls(
            episode_id=value.get("episode_id"), lesson_id=value.get("lesson_id"), concept_id=value.get("concept_id"),
            variant_id=value.get("variant_id"), domain=value.get("domain"), capability=value.get("capability"),
            risk_class=value.get("risk_class"), evaluator_id=value.get("evaluator_id"),
            evaluator_version=value.get("evaluator_version"), reward=value.get("reward"), passed=value.get("passed"),
            evidence_digest=value.get("evidence_digest"), lesson_digest=value.get("lesson_digest"),
            decision_digest=value.get("decision_digest"), observed_at=value.get("observed_at", 0.0),
            transferable=value.get("transferable", False),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_MEMORY_CONSOLIDATION_LESSON_SCHEMA,
            "episode_id": self.episode_id, "lesson_id": self.lesson_id, "concept_id": self.concept_id,
            "variant_id": self.variant_id, "domain": self.domain, "capability": self.capability,
            "risk_class": self.risk_class, "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version, "reward": self.reward, "passed": self.passed,
            "evidence_digest": self.evidence_digest, "lesson_digest": self.lesson_digest,
            "decision_digest": self.decision_digest, "observed_at": self.observed_at,
            "transferable": self.transferable, "retention": _RETENTION, "secret_material": _SECRET_MATERIAL,
        }


@dataclass(frozen=True, slots=True)
class AutonomousMemoryConsolidatedLesson:
    """Aggregate value-only reference eligible for bounded recall."""

    concept_id: str
    lesson_id: str
    variant_id: str
    scope: str
    domains: tuple[str, ...]
    capabilities: tuple[str, ...]
    risk_classes: tuple[str, ...]
    lesson_digest: str
    observation_count: int
    passed_count: int
    failed_count: int
    reward_mean: float
    support_lower_bound: float
    confidence: float
    first_observed_at: float
    last_observed_at: float
    transferable: bool
    status: str

    def __post_init__(self) -> None:
        for name in ("concept_id", "lesson_id", "variant_id"):
            object.__setattr__(self, name, _identifier(f"lesson {name}", getattr(self, name)))
        if self.scope not in _SCOPES:
            _fail("lesson scope is unsupported")
        object.__setattr__(self, "domains", tuple(_domain(value) for value in self.domains))
        object.__setattr__(self, "capabilities", _string_tuple("lesson capabilities", self.capabilities))
        object.__setattr__(self, "risk_classes", _string_tuple("lesson risk_classes", self.risk_classes))
        object.__setattr__(self, "lesson_digest", _digest("lesson lesson_digest", self.lesson_digest))
        count = _bounded_integer("lesson observation_count", self.observation_count, 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_OBSERVATIONS)
        passed = _bounded_integer("lesson passed_count", self.passed_count, 0, count)
        failed = _bounded_integer("lesson failed_count", self.failed_count, 0, count)
        if passed + failed > count:
            _fail("lesson passed and failed counts exceed observations")
        object.__setattr__(self, "observation_count", count)
        object.__setattr__(self, "passed_count", passed)
        object.__setattr__(self, "failed_count", failed)
        object.__setattr__(self, "reward_mean", _bounded_number("lesson reward_mean", self.reward_mean, 0.0, 1.0))
        object.__setattr__(self, "support_lower_bound", _bounded_number("lesson support_lower_bound", self.support_lower_bound, 0.0, 1.0))
        object.__setattr__(self, "confidence", _bounded_number("lesson confidence", self.confidence, 0.0, 1.0))
        object.__setattr__(self, "first_observed_at", _bounded_number("lesson first_observed_at", self.first_observed_at, 0.0, 9_223_372_036_854_775.0))
        object.__setattr__(self, "last_observed_at", _bounded_number("lesson last_observed_at", self.last_observed_at, self.first_observed_at, 9_223_372_036_854_775.0))
        object.__setattr__(self, "transferable", _boolean("lesson transferable", self.transferable))
        if self.status not in _STATUSES:
            _fail("lesson status is unsupported")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_MEMORY_CONSOLIDATION_LESSON_SCHEMA,
            "concept_id": self.concept_id, "lesson_id": self.lesson_id, "variant_id": self.variant_id,
            "scope": self.scope, "domains": list(self.domains), "capabilities": list(self.capabilities),
            "risk_classes": list(self.risk_classes), "lesson_digest": self.lesson_digest,
            "observation_count": self.observation_count, "passed_count": self.passed_count,
            "failed_count": self.failed_count, "reward_mean": self.reward_mean,
            "support_lower_bound": self.support_lower_bound, "confidence": self.confidence,
            "first_observed_at": self.first_observed_at, "last_observed_at": self.last_observed_at,
            "transferable": self.transferable, "status": self.status,
            "retention": _RETENTION, "secret_material": _SECRET_MATERIAL,
        }


class AutonomousMemoryConsolidationTextStore(Protocol):
    def read(self) -> str | None: ...
    def write(self, value: str) -> None: ...


class AutonomousMemoryConsolidationTransactionalTextStore(AutonomousMemoryConsolidationTextStore, Protocol):
    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool: ...


class AutonomousMemoryConsolidator:
    """Build and retain a bounded, evaluator-backed lesson index."""

    def __init__(
        self,
        *,
        min_observations: int = 3,
        min_support_lower_bound: float = 0.60,
        conflict_dominance: float = 0.75,
        max_age_seconds: float = 30 * 24 * 60 * 60,
        max_lessons: int = MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_LESSONS,
        clock: Callable[[], float] = time.time,
    ) -> None:
        self.min_observations = _bounded_integer("min_observations", min_observations, 1, 1_024)
        self.min_support_lower_bound = _bounded_number("min_support_lower_bound", min_support_lower_bound, 0.0, 1.0)
        self.conflict_dominance = _bounded_number("conflict_dominance", conflict_dominance, 0.5, 1.0)
        self.max_age_seconds = _bounded_number("max_age_seconds", max_age_seconds, 1.0, 31_536_000.0)
        self.max_lessons = _bounded_integer("max_lessons", max_lessons, 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_LESSONS)
        if not callable(clock):
            _fail("clock must be callable")
        self.clock = clock
        self._generation = 0
        self._previous_snapshot_digest: str | None = None
        self._report: dict[str, Any] | None = None

    @property
    def report(self) -> dict[str, Any] | None:
        return None if self._report is None else json.loads(canonical_json(self._report))

    def consolidate(
        self,
        observations: Sequence[Mapping[str, Any] | AutonomousMemoryConsolidationObservation],
        *,
        generation: int | None = None,
    ) -> dict[str, Any]:
        if not isinstance(observations, Sequence) or isinstance(observations, (str, bytes, bytearray)) or len(observations) > MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_OBSERVATIONS:
            _fail("observations exceed their bound")
        now = _bounded_number("consolidation clock", self.clock(), 0.0, 9_223_372_036_854_775.0)
        normalized = [item if isinstance(item, AutonomousMemoryConsolidationObservation) else AutonomousMemoryConsolidationObservation.from_mapping(item) for item in observations]
        identities: dict[tuple[str, str, str, str], AutonomousMemoryConsolidationObservation] = {}
        for item in normalized:
            identity = (item.episode_id, item.lesson_id, item.evaluator_id, item.evaluator_version)
            prior = identities.get(identity)
            if prior is not None and prior.to_dict() != item.to_dict():
                _fail(f"observation {item.episode_id} is contradictory for the same evaluator identity")
            identities[identity] = item
        rows = list(identities.values())
        groups: dict[tuple[str, str, str, str, str], list[AutonomousMemoryConsolidationObservation]] = {}
        for item in rows:
            scope_key = "cross_domain" if item.transferable else item.domain
            groups.setdefault((item.concept_id, item.lesson_id, item.variant_id, item.lesson_digest, scope_key), []).append(item)
        provisional: list[AutonomousMemoryConsolidatedLesson] = []
        for (concept_id, lesson_id, variant_id, lesson_digest, scope_key), items in sorted(groups.items()):
            domains = tuple(sorted({item.domain for item in items}))
            capabilities = tuple(sorted({item.capability for item in items}))
            risk_classes = tuple(sorted({item.risk_class for item in items}))
            count = len(items)
            passed = sum(1 for item in items if item.passed)
            failed = sum(1 for item in items if not item.passed)
            reward_mean = sum(_normalized_reward(item.reward) for item in items) / count
            support = _wilson_lower(passed, count)
            confidence = min(1.0, count / self.min_observations) * (0.5 + 0.5 * (passed / count))
            age = max(0.0, now - max(item.observed_at for item in items))
            status = "stale" if age > self.max_age_seconds else "stable" if count >= self.min_observations and support >= self.min_support_lower_bound else "candidate"
            provisional.append(AutonomousMemoryConsolidatedLesson(concept_id, lesson_id, variant_id, "cross_domain" if scope_key == "cross_domain" else "domain", domains, capabilities, risk_classes, lesson_digest, count, passed, failed, reward_mean, support, confidence, min(item.observed_at for item in items), max(item.observed_at for item in items), scope_key == "cross_domain", status))
        if len(provisional) > self.max_lessons:
            provisional.sort(key=lambda row: (-row.confidence, -row.reward_mean, row.concept_id, row.lesson_id, row.variant_id, row.scope))
            provisional = provisional[: self.max_lessons]
        by_concept: dict[tuple[str, str, str], list[AutonomousMemoryConsolidatedLesson]] = {}
        for row in provisional:
            scope_domain = row.domains[0] if row.scope == "domain" else "cross_domain"
            by_concept.setdefault((row.concept_id, row.scope, scope_domain), []).append(row)
        conflicts: list[dict[str, Any]] = []
        rewritten: list[AutonomousMemoryConsolidatedLesson] = []
        for key, variants in sorted(by_concept.items()):
            if len(variants) < 2:
                rewritten.extend(variants)
                continue
            support_mass = sum(max(0.0, row.reward_mean) * row.observation_count for row in variants)
            leader = max(variants, key=lambda row: (row.reward_mean, row.observation_count, row.variant_id))
            leader_mass = max(0.0, leader.reward_mean) * leader.observation_count
            conflict = support_mass <= 0 or leader_mass / support_mass < self.conflict_dominance
            if conflict:
                conflicts.append({"concept_id": key[0], "scope": key[1], "domain": key[2] if key[1] == "domain" else None, "variant_ids": [row.variant_id for row in sorted(variants, key=lambda item: item.variant_id)], "status": "conflicted"})
                rewritten.extend(AutonomousMemoryConsolidatedLesson(row.concept_id, row.lesson_id, row.variant_id, row.scope, row.domains, row.capabilities, row.risk_classes, row.lesson_digest, row.observation_count, row.passed_count, row.failed_count, row.reward_mean, row.support_lower_bound, row.confidence, row.first_observed_at, row.last_observed_at, row.transferable, "conflicted") for row in variants)
            else:
                rewritten.extend(variants)
        lessons = [row.to_dict() for row in sorted(rewritten, key=lambda row: (row.concept_id, row.scope, row.variant_id, row.lesson_id))]
        domain_rows = []
        for domain in _DOMAINS:
            domain_lessons = [row for row in rewritten if domain in row.domains]
            domain_rows.append({"domain": domain, "observation_count": sum(row.observation_count for row in domain_lessons), "lesson_count": len(domain_lessons), "stable_count": sum(1 for row in domain_lessons if row.status == "stable"), "conflicted_count": sum(1 for row in domain_lessons if row.status == "conflicted"), "portable_count": sum(1 for row in domain_lessons if row.scope == "cross_domain")})
        next_generation = self._generation + 1 if generation is None else _bounded_integer("generation", generation, 1, 2_147_483_647)
        if generation is not None and generation <= self._generation:
            _fail("generation must advance monotonically")
        body = {"schema": AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEMA, "generation": next_generation, "previous_report_digest": None if self._report is None else self._report["report_digest"], "policy": {"min_observations": self.min_observations, "min_support_lower_bound": self.min_support_lower_bound, "conflict_dominance": self.conflict_dominance, "max_age_seconds": self.max_age_seconds, "max_lessons": self.max_lessons}, "observation_count": len(normalized), "deduplicated_observation_count": len(identities), "lessons": lessons, "conflicts": conflicts, "domains": domain_rows, "retention": _RETENTION, "secret_material": _SECRET_MATERIAL}
        body["report_digest"] = content_digest(body)
        if len(canonical_json(body).encode("utf-8")) > MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SNAPSHOT_BYTES:
            _fail("report exceeds its byte bound")
        self._generation = next_generation
        self._report = body
        return json.loads(canonical_json(body))

    def recall(self, *, domain: str, capability: str | None = None, include_unstable: bool = False, limit: int = 8) -> list[dict[str, Any]]:
        domain = _domain(domain)
        if capability is not None:
            capability = _identifier("recall capability", capability)
        limit = _bounded_integer("recall limit", limit, 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_PROMPT_LESSONS)
        if self._report is None:
            return []
        rows = []
        for row in self._report["lessons"]:
            if domain not in row["domains"] or (capability is not None and capability not in row["capabilities"]):
                continue
            if row["status"] != "stable" and not include_unstable:
                continue
            rows.append(row)
        rows.sort(key=lambda row: (row["status"] != "stable", -row["confidence"], -row["reward_mean"], row["concept_id"], row["variant_id"]))
        return json.loads(canonical_json(rows[:limit]))

    def prompt_references(
        self,
        *,
        domain: str,
        capability: str | None = None,
        lesson_resolver: Callable[[str], str | None],
        limit: int = 8,
    ) -> list[dict[str, Any]]:
        """Resolve stable digest references transiently for a caller-owned prompt."""

        if not callable(lesson_resolver):
            _fail("lesson_resolver must be callable")
        references = []
        for row in self.recall(domain=domain, capability=capability, limit=limit):
            text = lesson_resolver(row["lesson_digest"])
            if text is None:
                continue
            if not isinstance(text, str) or not text.strip() or len(text.encode("utf-8")) > 4_096 or "\x00" in text:
                _fail("lesson_resolver returned malformed lesson text")
            references.append({"lesson_id": row["lesson_id"], "concept_id": row["concept_id"], "lesson_digest": row["lesson_digest"], "text": text, "status": row["status"], "confidence": row["confidence"], "source": "evaluator_gated_memory_consolidation"})
        return references

    def snapshot(self) -> dict[str, Any]:
        report = self._report
        if report is None:
            report = self.consolidate([])
        descriptor = {"schema": AUTONOMOUS_MEMORY_CONSOLIDATION_SNAPSHOT_SCHEMA, "generation": self._generation, "previous_snapshot_digest": self._previous_snapshot_digest, "report": report, "retention": _RETENTION, "secret_material": _SECRET_MATERIAL}
        snapshot = {**descriptor, "snapshot_digest": content_digest(descriptor)}
        if len(canonical_json(snapshot).encode("utf-8")) > MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SNAPSHOT_BYTES:
            _fail("snapshot exceeds its byte bound")
        self._previous_snapshot_digest = snapshot["snapshot_digest"]
        return json.loads(canonical_json(snapshot))

    def restore(self, snapshot: Mapping[str, Any]) -> dict[str, Any]:
        validated = validate_autonomous_memory_consolidation_snapshot(snapshot)
        policy = validated["report"]["policy"]
        expected = {"min_observations": self.min_observations, "min_support_lower_bound": self.min_support_lower_bound, "conflict_dominance": self.conflict_dominance, "max_age_seconds": self.max_age_seconds, "max_lessons": self.max_lessons}
        if policy != expected:
            _fail("restored policy conflicts with the configured consolidator")
        self._generation = validated["generation"]
        self._previous_snapshot_digest = validated["snapshot_digest"]
        self._report = validated["report"]
        return json.loads(canonical_json(self._report))


def validate_autonomous_memory_consolidation_report(value: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(value, Mapping) or value.get("schema") != AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEMA:
        _fail("report schema is invalid")
    if value.get("retention") != _RETENTION or value.get("secret_material") != _SECRET_MATERIAL:
        _fail("report retention markers are invalid")
    _bounded_integer("report generation", value.get("generation"), 1, 2_147_483_647)
    _bounded_integer("report observation_count", value.get("observation_count"), 0, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_OBSERVATIONS)
    _bounded_integer("report deduplicated_observation_count", value.get("deduplicated_observation_count"), 0, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_OBSERVATIONS)
    if value["deduplicated_observation_count"] > value["observation_count"]:
        _fail("report deduplicated count exceeds observation count")
    if not isinstance(value.get("policy"), Mapping) or set(value["policy"]) != {"min_observations", "min_support_lower_bound", "conflict_dominance", "max_age_seconds", "max_lessons"}:
        _fail("report policy is malformed")
    _bounded_integer("report policy min_observations", value["policy"]["min_observations"], 1, 1_024)
    _bounded_number("report policy min_support_lower_bound", value["policy"]["min_support_lower_bound"], 0, 1)
    _bounded_number("report policy conflict_dominance", value["policy"]["conflict_dominance"], 0.5, 1)
    _bounded_number("report policy max_age_seconds", value["policy"]["max_age_seconds"], 1, 31_536_000)
    _bounded_integer("report policy max_lessons", value["policy"]["max_lessons"], 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_LESSONS)
    lessons = value.get("lessons")
    if not isinstance(lessons, Sequence) or isinstance(lessons, (str, bytes, bytearray)) or len(lessons) > MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_LESSONS:
        _fail("report lessons are malformed")
    for raw in lessons:
        if not isinstance(raw, Mapping) or raw.get("schema") != AUTONOMOUS_MEMORY_CONSOLIDATION_LESSON_SCHEMA or raw.get("retention") != _RETENTION or raw.get("secret_material") != _SECRET_MATERIAL:
            _fail("report lesson is malformed")
        AutonomousMemoryConsolidatedLesson(raw.get("concept_id"), raw.get("lesson_id"), raw.get("variant_id"), raw.get("scope"), tuple(raw.get("domains", ())), tuple(raw.get("capabilities", ())), tuple(raw.get("risk_classes", ())), raw.get("lesson_digest"), raw.get("observation_count"), raw.get("passed_count"), raw.get("failed_count"), raw.get("reward_mean"), raw.get("support_lower_bound"), raw.get("confidence"), raw.get("first_observed_at"), raw.get("last_observed_at"), raw.get("transferable"), raw.get("status"))
    conflicts = value.get("conflicts")
    if not isinstance(conflicts, Sequence) or isinstance(conflicts, (str, bytes, bytearray)):
        _fail("report conflicts are malformed")
    for conflict in conflicts:
        if not isinstance(conflict, Mapping) or set(conflict) != {"concept_id", "scope", "domain", "variant_ids", "status"}:
            _fail("report conflict row is malformed")
        _identifier("conflict concept_id", conflict["concept_id"])
        if conflict["scope"] not in _SCOPES or conflict["status"] != "conflicted":
            _fail("report conflict row is malformed")
        conflict_domain = conflict["domain"]
        if conflict["scope"] == "domain":
            _domain(conflict_domain)
        elif conflict_domain is not None:
            _fail("report conflict domain scope is malformed")
        _string_tuple("conflict variant_ids", conflict["variant_ids"])
    domains = value.get("domains")
    if not isinstance(domains, Sequence) or len(domains) != len(_DOMAINS) or [row.get("domain") for row in domains] != list(_DOMAINS):
        _fail("report domain coverage must contain every built-in domain in canonical order")
    for row in domains:
        if not isinstance(row, Mapping) or row.get("domain") not in _DOMAINS:
            _fail("report domain coverage row is malformed")
        for field in ("observation_count", "lesson_count", "stable_count", "conflicted_count", "portable_count"):
            _bounded_integer(f"report domain {row.get('domain')} {field}", row.get(field), 0, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_OBSERVATIONS)
    _digest("report report_digest", value.get("report_digest"))
    body = dict(value)
    body.pop("report_digest", None)
    if content_digest(body) != value["report_digest"]:
        _fail("report digest does not match its canonical projection")
    return json.loads(canonical_json(value))


def validate_autonomous_memory_consolidation_snapshot(value: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(value, Mapping) or value.get("schema") != AUTONOMOUS_MEMORY_CONSOLIDATION_SNAPSHOT_SCHEMA:
        _fail("snapshot schema is invalid")
    if value.get("retention") != _RETENTION or value.get("secret_material") != _SECRET_MATERIAL:
        _fail("snapshot retention markers are invalid")
    _bounded_integer("snapshot generation", value.get("generation"), 1, 2_147_483_647)
    _digest("snapshot previous_snapshot_digest", value.get("previous_snapshot_digest"), optional=True)
    report = validate_autonomous_memory_consolidation_report(value.get("report"))
    _digest("snapshot snapshot_digest", value.get("snapshot_digest"))
    descriptor = {"schema": value["schema"], "generation": value["generation"], "previous_snapshot_digest": value["previous_snapshot_digest"], "report": report, "retention": value["retention"], "secret_material": value["secret_material"]}
    if content_digest(descriptor) != value["snapshot_digest"]:
        _fail("snapshot digest does not match its canonical projection")
    return json.loads(canonical_json(value))


class JsonAutonomousMemoryConsolidationPersistence:
    """Canonical JSON persistence over a caller-owned text store."""

    def __init__(self, text_store: AutonomousMemoryConsolidationTextStore, *, max_bytes: int = MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SNAPSHOT_BYTES) -> None:
        if not callable(getattr(text_store, "read", None)) or not callable(getattr(text_store, "write", None)):
            _fail("JSON text store is malformed")
        self.text_store = text_store
        self.max_bytes = _bounded_integer("JSON max_bytes", max_bytes, 1, MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SNAPSHOT_BYTES)

    def read(self) -> dict[str, Any] | None:
        encoded = self.text_store.read()
        if encoded is None:
            return None
        if not isinstance(encoded, str) or len(encoded.encode("utf-8")) > self.max_bytes:
            _fail("JSON snapshot exceeds its byte bound")
        try:
            parsed = json.loads(encoded)
        except (TypeError, ValueError, json.JSONDecodeError) as error:
            raise AutonomousMemoryConsolidationError("memory consolidation JSON is invalid") from error
        if canonical_json(parsed) != encoded:
            _fail("JSON snapshot is not canonical")
        return validate_autonomous_memory_consolidation_snapshot(parsed)

    def write(self, snapshot: Mapping[str, Any]) -> None:
        validated = validate_autonomous_memory_consolidation_snapshot(snapshot)
        encoded = canonical_json(validated)
        if len(encoded.encode("utf-8")) > self.max_bytes:
            _fail("JSON snapshot exceeds its byte bound")
        self.text_store.write(encoded)


class TransactionalJsonAutonomousMemoryConsolidationPersistence(JsonAutonomousMemoryConsolidationPersistence):
    def write_if_unchanged(self, expected_snapshot_digest: str | None, snapshot: Mapping[str, Any]) -> bool:
        _digest("expected_snapshot_digest", expected_snapshot_digest, optional=True)
        if not callable(getattr(self.text_store, "write_if_unchanged", None)):
            _fail("transactional JSON text store lacks compare-and-swap")
        validated = validate_autonomous_memory_consolidation_snapshot(snapshot)
        encoded = canonical_json(validated)
        if len(encoded.encode("utf-8")) > self.max_bytes:
            _fail("transactional JSON snapshot exceeds its byte bound")
        return bool(self.text_store.write_if_unchanged(expected_snapshot_digest, encoded))


class AutonomousMemoryConsolidationPersistenceCoordinator:
    """Serialize consolidator snapshots and fence stale writers."""

    def __init__(self, consolidator: AutonomousMemoryConsolidator, persistence: JsonAutonomousMemoryConsolidationPersistence) -> None:
        if not isinstance(consolidator, AutonomousMemoryConsolidator) or not callable(getattr(persistence, "read", None)) or not callable(getattr(persistence, "write", None)):
            _fail("persistence coordinator inputs are malformed")
        self.consolidator = consolidator
        self.persistence = persistence
        self.expected_snapshot_digest: str | None = None

    def restore(self) -> dict[str, Any] | None:
        snapshot = self.persistence.read()
        if snapshot is None:
            return None
        self.consolidator.restore(snapshot)
        self.expected_snapshot_digest = snapshot["snapshot_digest"]
        return snapshot

    def flush(self) -> dict[str, Any]:
        snapshot = self.consolidator.snapshot()
        if isinstance(self.persistence, TransactionalJsonAutonomousMemoryConsolidationPersistence):
            if not self.persistence.write_if_unchanged(self.expected_snapshot_digest, snapshot):
                _fail("persistence compare-and-swap conflict")
        else:
            self.persistence.write(snapshot)
        self.expected_snapshot_digest = snapshot["snapshot_digest"]
        return snapshot


__all__ = [
    "AUTONOMOUS_MEMORY_CONSOLIDATION_SCHEMA", "AUTONOMOUS_MEMORY_CONSOLIDATION_LESSON_SCHEMA",
    "AUTONOMOUS_MEMORY_CONSOLIDATION_SNAPSHOT_SCHEMA", "MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_OBSERVATIONS",
    "MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_LESSONS", "MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_DOMAINS",
    "MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_ID_BYTES", "MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_SNAPSHOT_BYTES",
    "MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_PROMPT_LESSONS", "AutonomousMemoryConsolidationError",
    "AutonomousMemoryConsolidationObservation", "AutonomousMemoryConsolidatedLesson",
    "AutonomousMemoryConsolidator", "AutonomousMemoryConsolidationTextStore",
    "AutonomousMemoryConsolidationTransactionalTextStore", "JsonAutonomousMemoryConsolidationPersistence",
    "TransactionalJsonAutonomousMemoryConsolidationPersistence", "AutonomousMemoryConsolidationPersistenceCoordinator",
    "validate_autonomous_memory_consolidation_report", "validate_autonomous_memory_consolidation_snapshot",
]
