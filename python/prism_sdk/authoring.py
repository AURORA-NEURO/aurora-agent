"""Authoring contracts for packs, decision cells, and metamorphic mutation plans.

This module is intentionally an authoring layer, not a second scientific kernel.  It helps a
notebook or agent construct the exact JSON shapes owned by the Rust ``packs``, ``prism``, and
``mutation`` crates, validates the parts that can be checked locally, and computes the same
canonical SHA-256 address used by ``bioprism-ids``.  Final health, oracle, mutation, and release
decisions still belong to the Rust tools and are exposed by :class:`Workspace` helpers.
"""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import math
import re
from typing import Any, Iterable, Mapping, Sequence

from .errors import ArgumentError

JsonObject = dict[str, Any]
JsonValue = Any

CELL_SCHEMA_VERSION = "bioprism-decision-cell/0.1"
_PACK_ID = re.compile(r"^[a-z][a-z0-9.-]*\.[a-z0-9.-]+$")
_MODULE_ID = re.compile(r"^[0-9]{2}\.[0-9]{2}$")
_DIGEST = re.compile(r"^[0-9a-f]{64}$")
_MECHANISMS = ("identity", "site", "temporal", "preprocessing")
_CAPABILITY_FAMILIES = {
    "agent": {
        "evidence_acquisition",
        "tool_use",
        "memory",
        "hypothesis_and_planning",
        "verification_and_recovery",
        "long_horizon_state",
        "coordination",
        "safety",
        "privacy",
        "evaluation_integrity",
        "robustness",
        "routing",
        "human_collaboration",
        "observability",
    },
    "biology": {
        "research_orientation",
        "data_identity",
        "assay_understanding",
        "quality_control",
        "cohort_and_study_design",
        "statistical_and_causal_inference",
        "computational_reproducibility",
        "interpretation_and_hypothesis",
        "experiment_and_evidence",
        "multimodal_translation",
        "model_development",
        "verification_and_abstention",
        "collaboration_and_governance",
    },
}
_DOMAINS = {
    "coding",
    "browser",
    "data",
    "science",
    "biomedical",
    "neuroscience",
    "operations",
    "enterprise",
    "multi_agent",
    "multimodal",
    "cross_domain",
    "evaluation",
}
_ORACLES = {"deterministic", "executable", "policy_veto", "statistical", "expert_review", "rubric"}
_MUTATION_KINDS = {
    "rename_subjects",
    "reorder_facts",
    "add_distractors",
    "camouflage_tags",
    "remove_leakage",
    "inject_leakage",
}


class AuthoringError(ArgumentError):
    """A local authoring document is unsafe, incomplete, or internally inconsistent."""


@dataclass(frozen=True)
class ValidationIssue:
    """One deterministic validation finding, suitable for a notebook or CI report."""

    path: str
    code: str
    message: str
    severity: str = "error"


@dataclass(frozen=True)
class ValidationReport:
    """All local findings; no finding is silently discarded after the first error."""

    issues: tuple[ValidationIssue, ...] = ()

    @property
    def errors(self) -> tuple[ValidationIssue, ...]:
        return tuple(issue for issue in self.issues if issue.severity == "error")

    @property
    def warnings(self) -> tuple[ValidationIssue, ...]:
        return tuple(issue for issue in self.issues if issue.severity == "warning")

    @property
    def ok(self) -> bool:
        return not self.errors

    def raise_if_invalid(self) -> None:
        if not self.ok:
            details = "; ".join(f"{issue.path}: {issue.message}" for issue in self.errors)
            raise AuthoringError(f"authoring document is invalid: {details}")


def canonical_json(value: JsonValue) -> str:
    """Return the repository's canonical JSON text, rejecting values Rust cannot represent."""

    _validate_json_value(value)
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False, allow_nan=False)


def canonical_bytes(value: JsonValue) -> bytes:
    return canonical_json(value).encode("utf-8")


def content_digest(value: JsonValue) -> str:
    """Compute the same lowercase SHA-256 content address as ``bioprism_ids``."""

    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def _clone(value: JsonValue) -> JsonValue:
    return json.loads(canonical_json(value))


def _validate_json_value(value: JsonValue, path: str = "$", depth: int = 0) -> None:
    if depth > 100:
        raise AuthoringError(f"{path}: JSON nesting exceeds 100 levels")
    if value is None or isinstance(value, (str, bool, int)):
        if isinstance(value, int) and not isinstance(value, bool):
            if value < -(2**63) or value > 2**64 - 1:
                raise AuthoringError(f"{path}: integer is outside Rust JSON's signed/unsigned range")
        return
    if isinstance(value, float):
        if not math.isfinite(value):
            raise AuthoringError(f"{path}: non-finite numbers are not canonical JSON")
        return
    if isinstance(value, Mapping):
        for key, child in value.items():
            if not isinstance(key, str):
                raise AuthoringError(f"{path}: JSON object keys must be strings")
            _validate_json_value(child, f"{path}.{key}", depth + 1)
        return
    if isinstance(value, (list, tuple)):
        for index, child in enumerate(value):
            _validate_json_value(child, f"{path}[{index}]", depth + 1)
        return
    raise AuthoringError(f"{path}: unsupported JSON value {type(value).__name__}")


def _text(value: str, path: str, *, max_bytes: int = 4096, allow_empty: bool = False) -> str:
    if not isinstance(value, str) or (not allow_empty and not value.strip()):
        raise AuthoringError(f"{path}: expected a non-empty string")
    if "\r" in value or "\n" in value or len(value.encode("utf-8")) > max_bytes:
        raise AuthoringError(f"{path}: value is not line-safe or exceeds {max_bytes} UTF-8 bytes")
    return value


def _nonnegative_int(value: int, path: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise AuthoringError(f"{path}: expected a non-negative integer")
    return value


@dataclass(frozen=True)
class InputRef:
    """A digest-bound world or query reference used by a decision cell."""

    locator: str
    sha256: str

    def __post_init__(self) -> None:
        _text(self.locator, "locator", max_bytes=2048)
        _require_digest(self.sha256, "sha256")

    @classmethod
    def from_document(cls, locator: str, document: JsonValue) -> "InputRef":
        return cls(_text(locator, "locator", max_bytes=2048), content_digest(document))

    @classmethod
    def from_digest(cls, locator: str, sha256: str) -> "InputRef":
        locator = _text(locator, "locator", max_bytes=2048)
        if not isinstance(sha256, str) or not _DIGEST.fullmatch(sha256):
            raise AuthoringError("sha256: expected a lowercase 64-character hexadecimal digest")
        return cls(locator, sha256)

    def to_dict(self) -> JsonObject:
        return {"locator": self.locator, "sha256": self.sha256}


@dataclass(frozen=True)
class AcceptanceResult:
    """Set-valued decision-cell evaluation; missing witnesses never become a partial pass."""

    passed: bool
    reason: str
    observed_verdict: str
    missing_witnesses: tuple[str, ...] = ()


@dataclass(frozen=True)
class DecisionCell:
    """A content-addressed, set-valued evaluation contract."""

    cell_id: str
    decision_point: str
    world: InputRef
    query: InputRef
    acceptable_verdicts: frozenset[str] = frozenset()
    required_witnesses: frozenset[str] = frozenset()
    require_protected_closure: bool = True
    schema_version: str = CELL_SCHEMA_VERSION

    def __post_init__(self) -> None:
        _text(self.cell_id, "cell_id", max_bytes=256)
        _text(self.decision_point, "decision_point", max_bytes=4096)
        if not isinstance(self.world, InputRef) or not isinstance(self.query, InputRef):
            raise AuthoringError("world and query must be InputRef values")
        if not isinstance(self.require_protected_closure, bool):
            raise AuthoringError("require_protected_closure must be boolean")
        _text(self.schema_version, "schema_version", max_bytes=256)

    def to_dict(self) -> JsonObject:
        return {
            "schema_version": self.schema_version,
            "cell_id": self.cell_id,
            "decision_point": self.decision_point,
            "world": self.world.to_dict(),
            "query": self.query.to_dict(),
            "acceptable_verdicts": sorted(self.acceptable_verdicts),
            "required_witnesses": sorted(self.required_witnesses),
            "require_protected_closure": self.require_protected_closure,
        }

    @property
    def digest(self) -> str:
        return content_digest(self.to_dict())

    def accepts(
        self,
        observed_verdict: str,
        witnesses: Iterable[str],
        closure_complete: bool,
    ) -> AcceptanceResult:
        observed_verdict = _text(observed_verdict, "observed_verdict", max_bytes=256)
        witness_set = {_text(value, "witness", max_bytes=256) for value in witnesses}
        if self.acceptable_verdicts and observed_verdict not in self.acceptable_verdicts:
            return AcceptanceResult(False, "wrong_verdict", observed_verdict)
        missing = tuple(sorted(self.required_witnesses - witness_set))
        if missing:
            return AcceptanceResult(False, "missing_witnesses", observed_verdict, missing)
        if self.require_protected_closure and not closure_complete:
            return AcceptanceResult(False, "closure_incomplete", observed_verdict)
        return AcceptanceResult(True, "passed", observed_verdict)


class DecisionCellBuilder:
    """Fluent builder that makes the set-valued acceptance clauses explicit."""

    def __init__(self, cell_id: str, decision_point: str, world: InputRef, query: InputRef) -> None:
        self._cell_id = _text(cell_id, "cell_id", max_bytes=256)
        self._decision_point = _text(decision_point, "decision_point", max_bytes=4096)
        self._world = world
        self._query = query
        self._acceptable: set[str] = set()
        self._witnesses: set[str] = set()
        self._require_closure = True

    def accepting(self, *verdicts: str) -> "DecisionCellBuilder":
        self._acceptable.update(_text(value, "acceptable_verdict", max_bytes=256) for value in verdicts)
        return self

    def requiring_witness(self, *witnesses: str) -> "DecisionCellBuilder":
        self._witnesses.update(_text(value, "required_witness", max_bytes=256) for value in witnesses)
        return self

    def protected_closure(self, required: bool = True) -> "DecisionCellBuilder":
        if not isinstance(required, bool):
            raise AuthoringError("require_protected_closure must be boolean")
        self._require_closure = required
        return self

    def build(self) -> DecisionCell:
        return DecisionCell(
            cell_id=self._cell_id,
            decision_point=self._decision_point,
            world=self._world,
            query=self._query,
            acceptable_verdicts=frozenset(self._acceptable),
            required_witnesses=frozenset(self._witnesses),
            require_protected_closure=self._require_closure,
        )


@dataclass(frozen=True)
class PackArtifact:
    """Validated PackIr JSON plus its immutable content address."""

    _document: JsonObject
    digest: str

    @classmethod
    def from_document(cls, document: Mapping[str, Any]) -> "PackArtifact":
        normalized = _clone(dict(document))
        if not isinstance(normalized, dict):
            raise AuthoringError("pack: expected a JSON object")
        validate_pack(normalized).raise_if_invalid()
        return cls(_document=normalized, digest=content_digest(normalized))

    @property
    def document(self) -> JsonObject:
        """Return a defensive copy so callers cannot invalidate ``digest`` by mutation."""

        return _clone(self._document)

    def to_json(self) -> str:
        return canonical_json(self._document)

    def to_mcp_arguments(
        self,
        observations: Mapping[str, Any],
        policy: Mapping[str, Any] | None = None,
    ) -> JsonObject:
        arguments: JsonObject = {"pack": _clone(self._document), "observations": _clone(dict(observations))}
        if policy is not None:
            arguments["policy"] = _clone(dict(policy))
        return arguments

    @property
    def counts(self) -> JsonObject:
        content = self._document["content"]
        parents = content["parent_environments"]
        instances = content["instances"]
        kind = instances["kind"]
        declared = instances.get("declared", instances.get("validated", 0))
        return {
            "parent_environments": len(parents),
            "decision_parents": sum(parent["decision_parents"] for parent in parents),
            "declared_instances": declared,
            "validated_instances": instances["validated"],
            "effective_sample_size": content["effective_sample_size"],
            "executed_trials": content["executed_trials"],
            "independent_reproductions": content["independent_reproductions"],
            "instance_source": kind,
        }


class PackBuilder:
    """Construct the exact serialized ``bioprism-packs::PackIr`` shape."""

    def __init__(
        self,
        *,
        pack_id: str,
        version: tuple[int, int, int],
        schema_range: tuple[int, int],
        title: str,
        measures: str,
        blueprint_module: str,
        axis: str,
        capabilities: Sequence[Mapping[str, str]],
        domains: Sequence[str],
        owners: Sequence[str],
        license: str,
    ) -> None:
        self.pack_id = _text(pack_id, "manifest.id", max_bytes=256)
        self.version = _version_tuple(version, "manifest.version")
        self.schema_range = _version_tuple(schema_range, "manifest.schema_range", allow_two=True)
        self.title = _text(title, "manifest.title")
        self.measures = _text(measures, "manifest.measures")
        self.blueprint_module = _text(blueprint_module, "manifest.blueprint_module", max_bytes=32)
        self.axis = _text(axis, "manifest.axis", max_bytes=64)
        self.capabilities = _clone(list(capabilities))
        self.domains = list(domains)
        self.owners = list(owners)
        self.license = _text(license, "manifest.license", max_bytes=256)
        self.dependencies: list[JsonObject] = []
        self.parents: list[JsonObject] = []
        self.decision_families: list[str] = []
        self.mutation_relations: list[str] = []
        self.oracles: list[str] = []
        self.instances: JsonObject | None = None
        self.executed_trials = 0
        self.independent_reproductions = 0
        self.effective_sample_size: int | None = None

    def dependency(self, pack_id: str, digest: str) -> "PackBuilder":
        pack_id = _text(pack_id, "dependency.id", max_bytes=256)
        _require_digest(digest, "dependency.digest")
        self.dependencies.append({"id": pack_id, "digest": digest})
        return self

    def parent(self, world: str, decision_parents: int) -> "PackBuilder":
        self.parents.append(
            {"world": _text(world, "parent.world", max_bytes=512), "decision_parents": _nonnegative_int(decision_parents, "parent.decision_parents")}
        )
        return self

    def decision_family(self, family: str) -> "PackBuilder":
        self.decision_families.append(_text(family, "decision_family", max_bytes=512))
        return self

    def mutation_relation(self, relation: str) -> "PackBuilder":
        self.mutation_relations.append(_text(relation, "mutation_relation", max_bytes=256))
        return self

    def oracle(self, tier: str) -> "PackBuilder":
        self.oracles.append(_text(tier, "oracle", max_bytes=128))
        return self

    def authored_instances(self, validated: int) -> "PackBuilder":
        self.instances = {"kind": "authored", "validated": _nonnegative_int(validated, "instances.validated")}
        return self

    def deterministic_instances(self, start: int, end_exclusive: int, declared: int, validated: int) -> "PackBuilder":
        self.instances = {
            "kind": "deterministic_generator",
            "seeds": {"start": _nonnegative_int(start, "instances.seeds.start"), "end_exclusive": _nonnegative_int(end_exclusive, "instances.seeds.end_exclusive")},
            "declared": _nonnegative_int(declared, "instances.declared"),
            "validated": _nonnegative_int(validated, "instances.validated"),
        }
        return self

    def adapter_instances(self, adapter: str, declared: int, validated: int) -> "PackBuilder":
        self.instances = {
            "kind": "adapter_import",
            "adapter": _text(adapter, "instances.adapter", max_bytes=256),
            "declared": _nonnegative_int(declared, "instances.declared"),
            "validated": _nonnegative_int(validated, "instances.validated"),
        }
        return self

    def trial_counts(self, executed: int, independent_reproductions: int) -> "PackBuilder":
        self.executed_trials = _nonnegative_int(executed, "content.executed_trials")
        self.independent_reproductions = _nonnegative_int(independent_reproductions, "content.independent_reproductions")
        return self

    def effective_sample(self, size: int | None) -> "PackBuilder":
        self.effective_sample_size = None if size is None else _nonnegative_int(size, "content.effective_sample_size")
        return self

    def document(self) -> JsonObject:
        return {
            "manifest": {
                "id": self.pack_id,
                "version": {"major": self.version[0], "minor": self.version[1], "patch": self.version[2]},
                "schema_range": {"min_inclusive": self.schema_range[0], "max_inclusive": self.schema_range[1]},
                "title": self.title,
                "measures": self.measures,
                "blueprint_module": self.blueprint_module,
                "axis": self.axis,
                "capabilities": list(self.capabilities),
                "domains": list(self.domains),
                "owners": list(self.owners),
                "license": self.license,
                "dependencies": list(self.dependencies),
            },
            "content": {
                "parent_environments": list(self.parents),
                "decision_families": list(self.decision_families),
                "mutation_relations": list(self.mutation_relations),
                "oracles": list(self.oracles),
                "instances": _clone(self.instances) if self.instances is not None else None,
                "executed_trials": self.executed_trials,
                "independent_reproductions": self.independent_reproductions,
                "effective_sample_size": self.effective_sample_size,
            },
        }

    def validate(self) -> ValidationReport:
        return validate_pack(self.document())

    def build(self) -> PackArtifact:
        return PackArtifact.from_document(self.document())


def validate_pack(document: Mapping[str, Any]) -> ValidationReport:
    """Validate PackIr's cross-field invariants without contacting a server."""

    issues: list[ValidationIssue] = []

    def error(path: str, code: str, message: str) -> None:
        issues.append(ValidationIssue(path, code, message))

    try:
        _validate_json_value(document)
    except AuthoringError as exc:
        error("$", "invalid_json", str(exc))
        return ValidationReport(tuple(issues))
    manifest = document.get("manifest") if isinstance(document, Mapping) else None
    content = document.get("content") if isinstance(document, Mapping) else None
    if not isinstance(manifest, Mapping):
        error("manifest", "missing_manifest", "must be an object")
    if not isinstance(content, Mapping):
        error("content", "missing_content", "must be an object")
    if issues:
        return ValidationReport(tuple(issues))

    pack_id = manifest.get("id")
    if not isinstance(pack_id, str) or not _PACK_ID.fullmatch(pack_id) or ".." in pack_id or pack_id.endswith("."):
        error("manifest.id", "malformed_pack_id", "must be lowercase namespace.name syntax")
    version = manifest.get("version")
    _validate_version(version, "manifest.version", error)
    schema = manifest.get("schema_range")
    if not isinstance(schema, Mapping) or not isinstance(schema.get("min_inclusive"), int) or not isinstance(schema.get("max_inclusive"), int):
        error("manifest.schema_range", "invalid_schema_range", "must contain integer min_inclusive and max_inclusive")
    elif schema["min_inclusive"] > schema["max_inclusive"]:
        error("manifest.schema_range", "empty_schema_range", "minimum cannot exceed maximum")
    for field in ("title", "measures", "license"):
        if not isinstance(manifest.get(field), str) or not manifest[field].strip():
            error(f"manifest.{field}", "required", "must be a non-empty string")
    module = manifest.get("blueprint_module")
    if not isinstance(module, str) or not _MODULE_ID.fullmatch(module):
        error("manifest.blueprint_module", "invalid_module", "must have NN.MM form")
    if manifest.get("axis") not in {"mechanism", "domain", "platform"}:
        error("manifest.axis", "invalid_axis", "must be mechanism, domain, or platform")
    capabilities = manifest.get("capabilities")
    if not isinstance(capabilities, list) or not capabilities:
        error("manifest.capabilities", "required_list", "must be a non-empty list of capability objects")
    else:
        for index, capability in enumerate(capabilities):
            path = f"manifest.capabilities[{index}]"
            if not isinstance(capability, Mapping) or len(capability) != 1:
                error(path, "invalid_capability", "must be one externally tagged agent or biology capability")
                continue
            family, value = next(iter(capability.items()))
            if family not in _CAPABILITY_FAMILIES or value not in _CAPABILITY_FAMILIES[family]:
                error(path, "unknown_capability", "must use a known agent or biology taxonomy value")
    for field in ("domains", "owners"):
        values = manifest.get(field)
        if not isinstance(values, list) or not values or any(not isinstance(value, str) or not value.strip() for value in values):
            error(f"manifest.{field}", "required_list", "must be a non-empty list of strings")
        elif field == "domains" and any(value not in _DOMAINS for value in values):
            error("manifest.domains", "unknown_domain", "must use a known PackIr domain value")
    dependencies = manifest.get("dependencies")
    if not isinstance(dependencies, list):
        error("manifest.dependencies", "invalid_dependencies", "must be a list")
    else:
        seen: set[str] = set()
        for index, dependency in enumerate(dependencies):
            path = f"manifest.dependencies[{index}]"
            if not isinstance(dependency, Mapping):
                error(path, "invalid_dependency", "must be an object")
                continue
            dependency_id = dependency.get("id")
            if not isinstance(dependency_id, str) or dependency_id in seen:
                error(f"{path}.id", "duplicate_dependency", "must be a unique pack id")
            if isinstance(dependency_id, str):
                seen.add(dependency_id)
            if not isinstance(dependency.get("digest"), str) or not _DIGEST.fullmatch(dependency["digest"]):
                error(f"{path}.digest", "unpinned_dependency", "must be a lowercase 64-character digest")

    parents = content.get("parent_environments")
    if not isinstance(parents, list):
        error("content.parent_environments", "invalid_parents", "must be a list")
    else:
        for index, parent in enumerate(parents):
            path = f"content.parent_environments[{index}]"
            if not isinstance(parent, Mapping) or not isinstance(parent.get("world"), str) or not parent.get("world"):
                error(path, "invalid_parent", "must contain a non-empty world")
            if not isinstance(parent, Mapping) or not isinstance(parent.get("decision_parents"), int) or isinstance(parent.get("decision_parents"), bool) or parent["decision_parents"] < 0:
                error(f"{path}.decision_parents", "invalid_count", "must be a non-negative integer")
    for field in ("decision_families", "mutation_relations", "oracles"):
        values = content.get(field)
        if not isinstance(values, list) or not values or any(not isinstance(value, str) or not value.strip() for value in values):
            error(f"content.{field}", "required_list", "must be a non-empty list of strings")
        elif field == "oracles" and any(value not in _ORACLES for value in values):
            error("content.oracles", "unknown_oracle", "must use a known PackIr oracle tier")
    instances = content.get("instances")
    if not isinstance(instances, Mapping):
        error("content.instances", "missing_instances", "must be configured explicitly")
    else:
        kind = instances.get("kind")
        if kind not in {"authored", "deterministic_generator", "adapter_import"}:
            error("content.instances.kind", "invalid_instance_source", "unknown instance source")
        declared = instances.get("declared", instances.get("validated"))
        validated = instances.get("validated")
        if not isinstance(declared, int) or isinstance(declared, bool) or declared < 0:
            error("content.instances.declared", "invalid_count", "must be a non-negative integer")
        if not isinstance(validated, int) or isinstance(validated, bool) or validated < 0:
            error("content.instances.validated", "invalid_count", "must be a non-negative integer")
        if isinstance(declared, int) and isinstance(validated, int) and validated > declared:
            error("content.instances.validated", "validated_exceeds_declared", "cannot exceed declared instances")
        if kind == "deterministic_generator":
            seeds = instances.get("seeds")
            if not isinstance(seeds, Mapping) or not isinstance(seeds.get("start"), int) or not isinstance(seeds.get("end_exclusive"), int):
                error("content.instances.seeds", "invalid_seed_range", "must contain integer start and end_exclusive")
            elif seeds["start"] < 0 or seeds["end_exclusive"] < 0:
                error("content.instances.seeds", "invalid_seed_range", "seed bounds must be non-negative")
            elif seeds["start"] > seeds["end_exclusive"]:
                error("content.instances.seeds", "inverted_seed_range", "start cannot exceed end_exclusive")
            elif isinstance(declared, int) and declared > seeds["end_exclusive"] - seeds["start"]:
                error("content.instances.declared", "counts_exceed_generator", "declared count exceeds seed capacity")
        if kind == "adapter_import" and (not isinstance(instances.get("adapter"), str) or not instances.get("adapter")):
            error("content.instances.adapter", "missing_adapter", "must be a non-empty string")
    for field in ("executed_trials", "independent_reproductions"):
        if not isinstance(content.get(field), int) or isinstance(content.get(field), bool) or content[field] < 0:
            error(f"content.{field}", "invalid_count", "must be a non-negative integer")
    effective = content.get("effective_sample_size")
    if effective is not None and (not isinstance(effective, int) or isinstance(effective, bool) or effective < 0):
        error("content.effective_sample_size", "invalid_count", "must be null or a non-negative integer")
    return ValidationReport(tuple(issues))


def _validate_version(value: Any, path: str, error: Any) -> None:
    if not isinstance(value, Mapping) or any(not isinstance(value.get(key), int) or isinstance(value.get(key), bool) or value[key] < 0 for key in ("major", "minor", "patch")):
        error(path, "invalid_version", "must contain non-negative integer major, minor, and patch")


def _version_tuple(value: Sequence[int], path: str, *, allow_two: bool = False) -> tuple[int, ...]:
    expected = 2 if allow_two else 3
    if not isinstance(value, (tuple, list)) or len(value) != expected:
        raise AuthoringError(f"{path}: expected {expected} non-negative integer components")
    return tuple(_nonnegative_int(component, f"{path}[{index}]") for index, component in enumerate(value))


@dataclass(frozen=True)
class MutationSpec:
    """A mutation and its declared postcondition, before any world is executed."""

    id: str
    kind: str
    mechanism: str | None = None
    prefix: str | None = None
    seed: int | None = None
    count: int | None = None

    def __post_init__(self) -> None:
        _text(self.id, "mutation.id", max_bytes=256)
        if self.kind not in _MUTATION_KINDS:
            raise AuthoringError(f"mutation.kind: unsupported kind {self.kind!r}")
        if self.kind in {"remove_leakage", "inject_leakage"}:
            if self.mechanism not in _MECHANISMS:
                raise AuthoringError("mutation.mechanism: expected identity, site, temporal, or preprocessing")
        if self.kind == "rename_subjects":
            _text(self.prefix or "", "mutation.prefix", max_bytes=128)
        if self.kind in {"reorder_facts", "add_distractors"}:
            if self.seed is None or _nonnegative_int(self.seed, "mutation.seed") is None:
                raise AuthoringError("mutation.seed: required non-negative integer")
        if self.kind == "add_distractors" and (self.count is None or _nonnegative_int(self.count, "mutation.count") is None):
            raise AuthoringError("mutation.count: required non-negative integer")

    @property
    def family(self) -> str:
        if self.kind == "rename_subjects":
            return "rename"
        if self.kind == "reorder_facts":
            return "reorder"
        if self.kind == "add_distractors":
            return "distractors"
        if self.kind == "camouflage_tags":
            return "camouflage"
        return f"{self.kind.removesuffix('_leakage')}-{self.mechanism}"

    @property
    def relation(self) -> JsonObject:
        if self.kind in {"rename_subjects", "reorder_facts", "add_distractors", "camouflage_tags"}:
            return {"relation": "preserves_verdict"}
        witness = f"{self.mechanism}_leakage"
        return {"relation": "removes_witness" if self.kind == "remove_leakage" else "adds_witness", "kind": witness}

    def to_dict(self) -> JsonObject:
        kind: JsonObject = {"kind": self.kind}
        if self.kind == "rename_subjects":
            kind["prefix"] = self.prefix
        elif self.kind == "reorder_facts":
            kind["seed"] = self.seed
        elif self.kind == "add_distractors":
            kind.update({"count": self.count, "seed": self.seed})
        elif self.kind in {"remove_leakage", "inject_leakage"}:
            kind["mechanism"] = self.mechanism
        return {"id": self.id, "kind": kind, "relation": self.relation}


@dataclass(frozen=True)
class MutationPlan:
    """Deterministic, duplicate-free mutation authoring plan."""

    mutations: tuple[MutationSpec, ...]

    @classmethod
    def standard(cls) -> "MutationPlan":
        mutations = [
            MutationSpec("rename-subjects", "rename_subjects", prefix="X"),
            MutationSpec("reorder-facts", "reorder_facts", seed=7),
            MutationSpec("add-distractors", "add_distractors", count=25, seed=11),
            MutationSpec("camouflage-tags", "camouflage_tags"),
        ]
        mutations.extend(
            MutationSpec(f"remove-{mechanism}", "remove_leakage", mechanism=mechanism)
            for mechanism in _MECHANISMS
        )
        return cls(tuple(mutations))

    def validate(self) -> ValidationReport:
        issues: list[ValidationIssue] = []
        ids = [mutation.id for mutation in self.mutations]
        if len(ids) != len(set(ids)):
            issues.append(ValidationIssue("mutations", "duplicate_id", "mutation ids must be unique"))
        if not self.mutations:
            issues.append(ValidationIssue("mutations", "empty_plan", "at least one mutation is required"))
        return ValidationReport(tuple(issues))

    def to_list(self) -> list[JsonObject]:
        self.validate().raise_if_invalid()
        return [mutation.to_dict() for mutation in self.mutations]

    def to_json(self) -> str:
        return canonical_json(self.to_list())

    def standard_tool_arguments(self, world: str, *, include_worlds: bool = False, max_worlds: int | None = None) -> JsonObject:
        if self != MutationPlan.standard():
            raise AuthoringError("mutation_family only accepts the Rust standard suite; custom plans are local declarations")
        arguments: JsonObject = {"world": _text(world, "world", max_bytes=2048), "suite": "standard", "include_worlds": include_worlds}
        if max_worlds is not None:
            arguments["max_worlds"] = _nonnegative_int(max_worlds, "max_worlds")
        return arguments


def _require_digest(value: str, path: str) -> None:
    if not isinstance(value, str) or not _DIGEST.fullmatch(value):
        raise AuthoringError(f"{path}: expected a lowercase 64-character hexadecimal digest")


__all__ = [
    "AcceptanceResult",
    "AuthoringError",
    "CELL_SCHEMA_VERSION",
    "DecisionCell",
    "DecisionCellBuilder",
    "InputRef",
    "MutationPlan",
    "MutationSpec",
    "PackArtifact",
    "PackBuilder",
    "ValidationIssue",
    "ValidationReport",
    "canonical_bytes",
    "canonical_json",
    "content_digest",
    "validate_pack",
]
