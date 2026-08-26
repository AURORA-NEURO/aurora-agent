"""Provider-free composition of every reviewed autonomous domain contract.

The operating kit is the metadata handoff between domain design and live execution. It binds
the profile, workflow, policy, task lens, response contract, prompt registry, evaluator, and
tool/evidence coverage without retaining task values or granting dispatch authority.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .authoring import content_digest
from .errors import ArgumentError
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES

AUTONOMOUS_DOMAIN_OPERATING_KIT_SCHEMA = "bioprism-python-autonomous-domain-operating-kit/0.1"
AUTONOMOUS_DOMAIN_OPERATING_KIT_STAGE_SCHEMA = "bioprism-python-autonomous-domain-operating-kit-stage/0.1"
AUTONOMOUS_DOMAIN_OPERATING_KIT_VERSION = "0.1"
MAX_AUTONOMOUS_DOMAIN_OPERATING_KITS = 12
MAX_AUTONOMOUS_DOMAIN_OPERATING_KIT_STAGES = 16
MAX_AUTONOMOUS_DOMAIN_OPERATING_KIT_CAPABILITIES = 128
MAX_AUTONOMOUS_DOMAIN_OPERATING_KIT_TOOLS = 128
_COVERAGE_KEYS = ("profile", "workflow", "policy", "task_lens", "response_contract", "prompt_templates", "evaluator", "stage_contracts", "tool_bindings")
_MARKERS = {
    "execution": "metadata_only; no_provider_source_tool_evaluator_or_effect_dispatch",
    "retention": "operating_contract_metadata_only;task_prompt_response_values_not_retained",
    "credential_posture": "caller_owned_opaque_handles_only;no_credentials_consumed",
    "secret_material": "never_returned",
}


def _identifier(name: str, value: Any) -> str:
    if not isinstance(value, str) or not value.strip() or len(value) > 256 or any(c not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.-" for c in value):
        raise ArgumentError(f"{name} must be a bounded identifier")
    return value


def _digest(name: str, value: Any) -> str:
    if not isinstance(value, str) or len(value) != 64 or any(c not in "0123456789abcdef" for c in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _strings(name: str, value: Any, *, maximum: int = MAX_AUTONOMOUS_DOMAIN_OPERATING_KIT_CAPABILITIES, required: bool = False) -> tuple[str, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)) or len(value) > maximum or (required and not value):
        raise ArgumentError(f"{name} is outside its bounded list contract")
    result = tuple(_identifier(f"{name} entry", item) for item in value)
    if len(set(result)) != len(result):
        raise ArgumentError(f"{name} must not contain duplicates")
    return result


def _texts(name: str, value: Any, *, maximum: int = 64) -> tuple[str, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)) or len(value) > maximum:
        raise ArgumentError(f"{name} is outside its bounded list contract")
    result = tuple(item.strip() for item in value if isinstance(item, str) and item.strip() and len(item.encode("utf-8")) <= 2_048)
    if len(result) != len(value):
        raise ArgumentError(f"{name} contains malformed text")
    return result


@dataclass(frozen=True, slots=True)
class AutonomousDomainOperatingKitStage:
    stage_id: str
    objective: str
    required_capabilities: tuple[str, ...]
    evidence_outputs: tuple[str, ...]
    evaluator_signals: tuple[str, ...]
    approval_required: bool
    read_only: bool
    prompt_candidate_ids: tuple[str, ...]
    selected_prompt_id: str | None
    selected_prompt_manifest_digest: str | None
    selected_prompt_version: str | None
    tool_names: tuple[str, ...]

    def __post_init__(self) -> None:
        _identifier("operating kit stage_id", self.stage_id)
        if not isinstance(self.objective, str) or not self.objective.strip() or len(self.objective) > 2_048:
            raise ArgumentError("operating kit stage objective is malformed")
        for name in ("required_capabilities", "evidence_outputs", "evaluator_signals", "prompt_candidate_ids", "tool_names"):
            values = _strings(f"operating kit stage {self.stage_id}.{name}", getattr(self, name), maximum=MAX_AUTONOMOUS_DOMAIN_OPERATING_KIT_TOOLS, required=name in {"required_capabilities", "evidence_outputs", "evaluator_signals"})
            object.__setattr__(self, name, values)
        if not isinstance(self.approval_required, bool) or not isinstance(self.read_only, bool):
            raise ArgumentError("operating kit stage safety flags are malformed")
        if self.selected_prompt_id is not None:
            _identifier("operating kit selected_prompt_id", self.selected_prompt_id)
        if self.selected_prompt_manifest_digest is not None:
            _digest("operating kit selected_prompt_manifest_digest", self.selected_prompt_manifest_digest)
        if self.selected_prompt_version is not None and (not isinstance(self.selected_prompt_version, str) or not self.selected_prompt_version.strip()):
            raise ArgumentError("operating kit selected_prompt_version is malformed")

    def _descriptor(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_DOMAIN_OPERATING_KIT_STAGE_SCHEMA,
            "stage_id": self.stage_id,
            "objective": self.objective,
            "required_capabilities": list(self.required_capabilities),
            "evidence_outputs": list(self.evidence_outputs),
            "evaluator_signals": list(self.evaluator_signals),
            "approval_required": self.approval_required,
            "read_only": self.read_only,
            "prompt_candidate_ids": list(self.prompt_candidate_ids),
            "selected_prompt_id": self.selected_prompt_id,
            "selected_prompt_manifest_digest": self.selected_prompt_manifest_digest,
            "selected_prompt_version": self.selected_prompt_version,
            "tool_names": list(self.tool_names),
        }

    @property
    def stage_digest(self) -> str:
        return content_digest(self._descriptor())

    def to_dict(self) -> dict[str, Any]:
        return {**self._descriptor(), "stage_digest": self.stage_digest}


@dataclass(frozen=True, slots=True)
class AutonomousDomainOperatingKit:
    domain: str
    profile_digest: str
    workflow_id: str
    workflow_digest: str
    domain_policy_digest: str
    task_lens_digest: str
    response_contract_digest: str
    prompt_registry_digest: str
    evaluator_id: str
    evaluator_version: str
    evaluator_profile_digest: str
    stages: tuple[AutonomousDomainOperatingKitStage, ...]
    capability_graph: tuple[Mapping[str, Any], ...]
    coverage: Mapping[str, bool]
    status: str
    next_actions: tuple[str, ...]

    def __post_init__(self) -> None:
        if self.domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError(f"unsupported autonomous operating-kit domain: {self.domain!r}")
        _identifier("operating kit workflow_id", self.workflow_id)
        _identifier("operating kit evaluator_id", self.evaluator_id)
        if not isinstance(self.evaluator_version, str) or not self.evaluator_version.strip():
            raise ArgumentError("operating kit evaluator_version is malformed")
        for name in ("profile_digest", "workflow_digest", "domain_policy_digest", "task_lens_digest", "response_contract_digest", "prompt_registry_digest", "evaluator_profile_digest"):
            _digest(f"operating kit {name}", getattr(self, name))
        if not isinstance(self.stages, Sequence) or isinstance(self.stages, (str, bytes, bytearray)) or not 1 <= len(self.stages) <= MAX_AUTONOMOUS_DOMAIN_OPERATING_KIT_STAGES or any(not isinstance(stage, AutonomousDomainOperatingKitStage) for stage in self.stages):
            raise ArgumentError("operating kit stages are outside their bounds")
        if len({stage.stage_id for stage in self.stages}) != len(self.stages):
            raise ArgumentError("operating kit stage ids must be unique")
        if not isinstance(self.capability_graph, Sequence) or isinstance(self.capability_graph, (str, bytes, bytearray)) or not 1 <= len(self.capability_graph) <= MAX_AUTONOMOUS_DOMAIN_OPERATING_KIT_CAPABILITIES:
            raise ArgumentError("operating kit capability_graph is outside its bounds")
        graph: list[Mapping[str, Any]] = []
        for row in self.capability_graph:
            if not isinstance(row, Mapping):
                raise ArgumentError("operating kit capability graph row is malformed")
            _identifier("operating kit capability", row.get("capability"))
            for name in ("stage_ids", "tool_names", "evaluator_signals", "evidence_outputs"):
                _strings(f"operating kit capability {name}", row.get(name), maximum=MAX_AUTONOMOUS_DOMAIN_OPERATING_KIT_TOOLS)
            graph.append(dict(row))
        if set(self.coverage) != set(_COVERAGE_KEYS) or any(not isinstance(self.coverage.get(key), bool) for key in _COVERAGE_KEYS):
            raise ArgumentError("operating kit coverage is malformed")
        if self.status not in {"complete", "partial", "blocked"}:
            raise ArgumentError("operating kit status is unsupported")
        object.__setattr__(self, "stages", tuple(self.stages))
        object.__setattr__(self, "capability_graph", tuple(graph))
        object.__setattr__(self, "coverage", {key: self.coverage[key] for key in _COVERAGE_KEYS})
        object.__setattr__(self, "next_actions", _texts("operating kit next_actions", self.next_actions))

    def _descriptor(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_DOMAIN_OPERATING_KIT_SCHEMA,
            "version": AUTONOMOUS_DOMAIN_OPERATING_KIT_VERSION,
            "domain": self.domain,
            "profile_digest": self.profile_digest,
            "workflow_id": self.workflow_id,
            "workflow_digest": self.workflow_digest,
            "domain_policy_digest": self.domain_policy_digest,
            "task_lens_digest": self.task_lens_digest,
            "response_contract_digest": self.response_contract_digest,
            "prompt_registry_digest": self.prompt_registry_digest,
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "evaluator_profile_digest": self.evaluator_profile_digest,
            "stages": [stage.to_dict() for stage in self.stages],
            "capability_graph": [
                {key: (list(row[key]) if key != "capability" else row[key]) for key in ("capability", "stage_ids", "tool_names", "evaluator_signals", "evidence_outputs")}
                for row in self.capability_graph
            ],
            "coverage": dict(self.coverage),
            "status": self.status,
            "next_actions": list(self.next_actions),
            **_MARKERS,
        }

    @property
    def kit_digest(self) -> str:
        return content_digest(self._descriptor())

    def to_dict(self) -> dict[str, Any]:
        return {**self._descriptor(), "kit_digest": self.kit_digest}

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousDomainOperatingKit":
        if not isinstance(value, Mapping):
            raise ArgumentError("autonomous operating kit must be a mapping")
        expected = set(cls._expected_keys())
        if set(value) != expected:
            raise ArgumentError("autonomous operating kit contains unsupported or missing fields")
        if value["schema"] != AUTONOMOUS_DOMAIN_OPERATING_KIT_SCHEMA or value["version"] != AUTONOMOUS_DOMAIN_OPERATING_KIT_VERSION or any(value[key] != marker for key, marker in _MARKERS.items()):
            raise ArgumentError("autonomous operating kit schema, version, or safety markers are malformed")
        stages: list[AutonomousDomainOperatingKitStage] = []
        for raw in value["stages"]:
            if not isinstance(raw, Mapping) or set(raw) != set(cls._expected_stage_keys()) or raw["schema"] != AUTONOMOUS_DOMAIN_OPERATING_KIT_STAGE_SCHEMA:
                raise ArgumentError("autonomous operating kit stage contains unsupported or missing fields")
            stage = AutonomousDomainOperatingKitStage(
                stage_id=raw["stage_id"], objective=raw["objective"], required_capabilities=tuple(raw["required_capabilities"]),
                evidence_outputs=tuple(raw["evidence_outputs"]), evaluator_signals=tuple(raw["evaluator_signals"]), approval_required=raw["approval_required"],
                read_only=raw["read_only"], prompt_candidate_ids=tuple(raw["prompt_candidate_ids"]), selected_prompt_id=raw["selected_prompt_id"],
                selected_prompt_manifest_digest=raw["selected_prompt_manifest_digest"], selected_prompt_version=raw["selected_prompt_version"], tool_names=tuple(raw["tool_names"]),
            )
            if raw["stage_digest"] != stage.stage_digest:
                raise ArgumentError(f"autonomous operating kit stage {stage.stage_id} digest does not match its contents")
            stages.append(stage)
        kit = cls(
            domain=value["domain"], profile_digest=value["profile_digest"], workflow_id=value["workflow_id"], workflow_digest=value["workflow_digest"],
            domain_policy_digest=value["domain_policy_digest"], task_lens_digest=value["task_lens_digest"], response_contract_digest=value["response_contract_digest"],
            prompt_registry_digest=value["prompt_registry_digest"], evaluator_id=value["evaluator_id"], evaluator_version=value["evaluator_version"],
            evaluator_profile_digest=value["evaluator_profile_digest"], stages=tuple(stages), capability_graph=tuple(value["capability_graph"]),
            coverage=value["coverage"], status=value["status"], next_actions=tuple(value["next_actions"]),
        )
        if value["kit_digest"] != kit.kit_digest:
            raise ArgumentError("autonomous operating kit digest does not match its contents")
        return kit

    @staticmethod
    def _expected_keys() -> tuple[str, ...]:
        return ("schema", "version", "domain", "profile_digest", "workflow_id", "workflow_digest", "domain_policy_digest", "task_lens_digest", "response_contract_digest", "prompt_registry_digest", "evaluator_id", "evaluator_version", "evaluator_profile_digest", "stages", "capability_graph", "coverage", "status", "next_actions", "execution", "retention", "credential_posture", "secret_material", "kit_digest")

    @staticmethod
    def _expected_stage_keys() -> tuple[str, ...]:
        return ("schema", "stage_id", "objective", "required_capabilities", "evidence_outputs", "evaluator_signals", "approval_required", "read_only", "prompt_candidate_ids", "selected_prompt_id", "selected_prompt_manifest_digest", "selected_prompt_version", "tool_names", "stage_digest")


def _resolve_domains(domains: Sequence[str] | None) -> tuple[str, ...]:
    result = tuple(AUTONOMOUS_DOMAIN_NAMES if domains is None else domains)
    if not 1 <= len(result) <= MAX_AUTONOMOUS_DOMAIN_OPERATING_KITS or len(set(result)) != len(result) or any(domain not in AUTONOMOUS_DOMAIN_NAMES for domain in result):
        raise ArgumentError("operating-kit domains are outside their bounds")
    return result


def _build(domain: str) -> AutonomousDomainOperatingKit:
    from .autonomy import _portfolio_binding_supports_stage, builtin_autonomous_domain_profiles, builtin_autonomous_workflow_strategies
    from .autonomous_domain_policy import autonomous_domain_policy
    from .autonomous_domain_response import build_autonomous_domain_response_contract
    from .autonomous_prompt_registry import builtin_autonomous_prompt_registry
    from .autonomous_task_lens import autonomous_domain_task_lens
    from .domain_tools import builtin_autonomous_domain_tool_profiles
    from .evaluators import builtin_autonomous_domain_evaluator_profiles

    profile = next((item for item in builtin_autonomous_domain_profiles() if item.domain == domain), None)
    workflow = next((item for item in builtin_autonomous_workflow_strategies() if item.domain == domain), None)
    evaluator = next((item for item in builtin_autonomous_domain_evaluator_profiles() if item.domain == domain), None)
    tool_profile = next((item for item in builtin_autonomous_domain_tool_profiles() if item.domain == domain), None)
    if profile is None or workflow is None or evaluator is None or tool_profile is None:
        raise ArgumentError(f"operating kit is missing a built-in contract for {domain!r}")
    policy = autonomous_domain_policy(domain)
    lens = autonomous_domain_task_lens(domain)
    contract = build_autonomous_domain_response_contract(profile, workflow=workflow)
    registry = builtin_autonomous_prompt_registry((domain,))
    stages: list[AutonomousDomainOperatingKitStage] = []
    for stage in workflow.stages:
        candidates = registry.candidates(domain, stage.id, ())
        selected = candidates[0] if candidates else None
        stages.append(
            AutonomousDomainOperatingKitStage(
                stage_id=stage.id, objective=stage.objective, required_capabilities=tuple(stage.required_capabilities), evidence_outputs=tuple(stage.evidence_outputs),
                evaluator_signals=tuple(stage.evaluator_signals), approval_required=stage.approval_required, read_only=stage.read_only,
                prompt_candidate_ids=tuple(item.manifest.prompt_id for item in candidates),
                selected_prompt_id=selected.manifest.prompt_id if selected else None,
                selected_prompt_manifest_digest=selected.manifest.manifest_digest if selected else None,
                selected_prompt_version=selected.manifest.version if selected else None,
                tool_names=tuple(sorted(binding.name for binding in tool_profile.bindings if _portfolio_binding_supports_stage(domain, stage, binding))),
            )
        )
    stages = tuple(stages)
    capabilities = sorted(set(profile.capabilities).union(*(set(stage.required_capabilities) for stage in stages)))
    graph = tuple({
        "capability": capability,
        "stage_ids": tuple(stage.stage_id for stage in stages if capability in stage.required_capabilities),
        "tool_names": tuple(sorted({name for stage in stages if capability in stage.required_capabilities for name in stage.tool_names})),
        "evaluator_signals": tuple(sorted({signal for stage in stages if capability in stage.required_capabilities for signal in stage.evaluator_signals})),
        "evidence_outputs": tuple(sorted({output for stage in stages if capability in stage.required_capabilities for output in stage.evidence_outputs})),
    } for capability in capabilities)
    coverage = {
        "profile": True, "workflow": bool(workflow.workflow_id and _digest("workflow_digest", workflow.workflow_digest)), "policy": bool(_digest("domain_policy_digest", policy.policy_digest)),
        "task_lens": bool(_digest("task_lens_digest", lens.lens_digest)), "response_contract": bool(_digest("response_contract_digest", contract.contract_digest)),
        "prompt_templates": bool(stages) and all(stage.selected_prompt_id is not None and stage.selected_prompt_manifest_digest is not None for stage in stages),
        "evaluator": evaluator.domain == domain and bool(evaluator.evaluator_id), "stage_contracts": bool(stages) and all(stage.required_capabilities and stage.evidence_outputs and stage.evaluator_signals for stage in stages),
        "tool_bindings": bool(tool_profile.bindings) and all(stage.tool_names for stage in stages),
    }
    failed = tuple(key for key in _COVERAGE_KEYS if not coverage[key])
    status = "blocked" if any(not coverage[key] for key in ("profile", "workflow", "policy", "response_contract", "evaluator")) else ("partial" if failed else "complete")
    next_actions = (("resolve caller-owned provider and credential handles", "run the ordinary route, selection, evidence, approval, and evaluator gates") if status == "complete" else tuple(f"repair_missing_{key}" for key in failed))
    tool_descriptor = tool_profile.to_dict()
    return AutonomousDomainOperatingKit(
        domain=domain, profile_digest=content_digest({"profile": profile.to_dict(), "workflow": workflow.to_dict(), "tool_profile": tool_descriptor}), workflow_id=workflow.workflow_id,
        workflow_digest=workflow.workflow_digest, domain_policy_digest=policy.policy_digest, task_lens_digest=lens.lens_digest, response_contract_digest=contract.contract_digest,
        prompt_registry_digest=registry.registry_digest, evaluator_id=evaluator.evaluator_id, evaluator_version=evaluator.evaluator_version, evaluator_profile_digest=content_digest(evaluator.to_dict()),
        stages=stages, capability_graph=graph, coverage=coverage, status=status, next_actions=next_actions,
    )


def build_autonomous_domain_operating_kit(domain: str) -> AutonomousDomainOperatingKit:
    if domain not in AUTONOMOUS_DOMAIN_NAMES:
        raise ArgumentError(f"unsupported autonomous operating-kit domain: {domain!r}")
    return _build(domain)


def build_autonomous_domain_operating_kits(domains: Sequence[str] | None = None) -> tuple[AutonomousDomainOperatingKit, ...]:
    return tuple(build_autonomous_domain_operating_kit(domain) for domain in _resolve_domains(domains))


def autonomous_domain_operating_kit(domain: str) -> AutonomousDomainOperatingKit:
    return build_autonomous_domain_operating_kit(domain)


def validate_autonomous_domain_operating_kit(value: AutonomousDomainOperatingKit | Mapping[str, Any]) -> AutonomousDomainOperatingKit:
    kit = value if isinstance(value, AutonomousDomainOperatingKit) else AutonomousDomainOperatingKit.from_dict(value)
    current = build_autonomous_domain_operating_kit(kit.domain)
    if current.kit_digest != kit.kit_digest or content_digest(kit._descriptor()) != kit.kit_digest:
        raise ArgumentError("autonomous operating kit is stale or tampered")
    return current


__all__ = [
    "AUTONOMOUS_DOMAIN_OPERATING_KIT_SCHEMA", "AUTONOMOUS_DOMAIN_OPERATING_KIT_STAGE_SCHEMA", "AUTONOMOUS_DOMAIN_OPERATING_KIT_VERSION",
    "MAX_AUTONOMOUS_DOMAIN_OPERATING_KITS", "MAX_AUTONOMOUS_DOMAIN_OPERATING_KIT_STAGES", "MAX_AUTONOMOUS_DOMAIN_OPERATING_KIT_CAPABILITIES", "MAX_AUTONOMOUS_DOMAIN_OPERATING_KIT_TOOLS",
    "AutonomousDomainOperatingKitStage", "AutonomousDomainOperatingKit", "build_autonomous_domain_operating_kit", "build_autonomous_domain_operating_kits", "autonomous_domain_operating_kit", "validate_autonomous_domain_operating_kit",
]
