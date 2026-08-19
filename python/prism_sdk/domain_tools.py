"""Domain-aware tool contracts for the autonomous agent façade.

The provider runtime only knows how to transport a function schema and continue a conversation
after a caller-approved result.  This module supplies the application-level composition that was
previously left to each embedding application:

* tools are registered with explicit domains, capabilities, and risk posture;
* the exact input schema is reused for local preflight and provider wire generation;
* read-only tools may be automatically executed by an embedding application;
* every effectful or approval-required tool still needs a separate caller approval callback; and
* receipts retain digests and statuses, never tool arguments, outputs, credentials, or provider
  payloads.

This is deliberately an execution adapter, not a new source of scientific or operational truth.
The registered executor remains caller-owned and the authoritative MCP tool may still refuse the
request.  A model intent never becomes permission merely because it appears in a tool schema.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Callable, Mapping, Sequence

from .authoring import content_digest
from .errors import ArgumentError
from .llm_runtime import ProviderTool, ProviderToolCall, ProviderToolResult
from .autonomy_persistence import (
    AutonomousExecutionController,
    AutonomousExecutionJournal,
    AutonomousExecutionPolicy,
    AutonomyPolicyError,
)
from .tooling import ToolCatalogue, ToolDefinition, ToolSchemaError


DOMAIN_TOOL_SCHEMA = "bioprism-python-autonomous-domain-tool/0.1"
DOMAIN_TOOL_REGISTRY_SCHEMA = "bioprism-python-autonomous-domain-tool-registry/0.1"
DOMAIN_TOOL_BINDING_SCHEMA = "bioprism-python-autonomous-domain-tool-binding/0.1"
DOMAIN_TOOL_BINDING_PLAN_SCHEMA = "bioprism-python-autonomous-domain-tool-binding-plan/0.1"
DOMAIN_TOOL_PROFILE_SCHEMA = "bioprism-python-autonomous-domain-tool-profile/0.1"
AUTONOMOUS_DOMAIN_NAMES = (
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
)
DOMAIN_TOOL_RISK_CLASSES = (
    "read_only",
    "reversible_effect",
    "external_effect",
    "high_impact_effect",
)
DOMAIN_TOOL_EXECUTION_STATUSES = (
    "executed",
    "approval_required",
    "policy_refused",
    "unknown_tool",
    "schema_refused",
    "execution_failed",
)
MAX_DOMAIN_TOOLS = 512
MAX_DOMAIN_TOOL_DOMAINS = 32
MAX_DOMAIN_TOOL_DESCRIPTION_BYTES = 16_000
MAX_DOMAIN_TOOL_RESULT_BYTES = 1_000_000
MAX_DOMAIN_TOOL_CALLS = 128
MAX_DOMAIN_TOOL_BINDING_PLAN_BYTES = 512_000
_SAFE_IDENTIFIER_CHARS = frozenset(
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.-"
)
_SECRET_FIELD_MARKERS = frozenset(
    {
        "apikey",
        "authorization",
        "bearer",
        "credential",
        "password",
        "refreshtoken",
        "secret",
        "accesstoken",
        "token",
        "privatekey",
        "secretkey",
        "clientsecret",
    }
)


def _text(name: str, value: Any, *, maximum: int = 512) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise ArgumentError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} exceeds its bounded size")
    return value


def _identifier(name: str, value: Any) -> str:
    result = _text(name, value, maximum=256)
    if any(character not in _SAFE_IDENTIFIER_CHARS for character in result):
        raise ArgumentError(f"{name} must be a bounded identifier")
    return result


def _sequence(name: str, value: Any, *, maximum: int) -> tuple[str, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be a sequence")
    if not value or len(value) > maximum:
        raise ArgumentError(f"{name} must contain between 1 and {maximum} entries")
    result: list[str] = []
    seen: set[str] = set()
    for item in value:
        item_text = _identifier(f"{name} entry", item)
        if item_text in seen:
            raise ArgumentError(f"{name} contains a duplicate entry: {item_text}")
        seen.add(item_text)
        result.append(item_text)
    return tuple(result)


def _json_safe(name: str, value: Any, *, maximum: int) -> Any:
    try:
        encoded = json.dumps(
            value,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        )
    except (TypeError, ValueError) as error:
        raise ArgumentError(f"{name} must be JSON-safe") from error
    if len(encoded.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} exceeds its bounded size")
    return json.loads(encoded)


def _reject_secret_fields(value: Any, *, depth: int = 0) -> None:
    if depth > 32:
        raise ArgumentError("domain tool value is too deeply nested")
    if isinstance(value, Mapping):
        for key, child in value.items():
            if isinstance(key, str):
                normalized = "".join(character for character in key.lower() if character.isalnum())
                if normalized in _SECRET_FIELD_MARKERS:
                    raise ArgumentError("domain tool values cannot contain credential-shaped fields")
            _reject_secret_fields(child, depth=depth + 1)
    elif isinstance(value, Sequence) and not isinstance(value, (str, bytes)):
        for child in value:
            _reject_secret_fields(child, depth=depth + 1)


@dataclass(frozen=True, slots=True)
class AutonomousDomainTool:
    """One provider-visible tool contract with explicit domain and effect posture."""

    name: str
    domains: tuple[str, ...]
    capability: str
    description: str
    parameters: Mapping[str, Any]
    risk_class: str = "read_only"
    read_only: bool = True
    approval_required: bool = False

    def __post_init__(self) -> None:
        name = _identifier("domain tool name", self.name)
        domains = _sequence("domain tool domains", self.domains, maximum=MAX_DOMAIN_TOOL_DOMAINS)
        capability = _identifier("domain tool capability", self.capability)
        description = _text(
            "domain tool description",
            self.description,
            maximum=MAX_DOMAIN_TOOL_DESCRIPTION_BYTES,
        )
        if self.risk_class not in DOMAIN_TOOL_RISK_CLASSES:
            raise ArgumentError(
                "domain tool risk_class must be one of: " + ", ".join(DOMAIN_TOOL_RISK_CLASSES)
            )
        if not isinstance(self.read_only, bool) or not isinstance(self.approval_required, bool):
            raise ArgumentError("domain tool safety flags must be booleans")
        if self.read_only and self.risk_class != "read_only":
            raise ArgumentError("read-only tools must use risk_class=read_only")
        if not self.read_only and not self.approval_required:
            raise ArgumentError("effectful domain tools must require approval")
        if not isinstance(self.parameters, Mapping):
            raise ArgumentError("domain tool parameters must be a JSON object")
        parameters = _json_safe(
            "domain tool parameters",
            dict(self.parameters),
            maximum=256_000,
        )
        if not isinstance(parameters, dict):
            raise ArgumentError("domain tool parameters must be a JSON object")
        _reject_secret_fields(parameters)
        object.__setattr__(self, "name", name)
        object.__setattr__(self, "domains", domains)
        object.__setattr__(self, "capability", capability)
        object.__setattr__(self, "description", description)
        object.__setattr__(self, "parameters", parameters)

    @classmethod
    def from_mcp_definition(
        cls,
        definition: Mapping[str, Any] | ToolDefinition,
        *,
        domains: Sequence[str],
        capability: str,
        risk_class: str = "read_only",
        read_only: bool = True,
        approval_required: bool = False,
    ) -> "AutonomousDomainTool":
        source = definition if isinstance(definition, ToolDefinition) else ToolDefinition.from_mapping(definition)
        return cls(
            name=source.name,
            domains=tuple(domains),
            capability=capability,
            description=source.description or f"Invoke the bounded {source.name} tool.",
            parameters=source.input_schema,
            risk_class=risk_class,
            read_only=read_only,
            approval_required=approval_required,
        )

    @property
    def schema_digest(self) -> str:
        return content_digest(dict(self.parameters))

    def to_provider_tool(self) -> ProviderTool:
        posture = "read-only" if self.read_only else f"{self.risk_class}; caller approval required"
        return ProviderTool(
            name=self.name,
            description=f"[{posture}] {self.description}",
            parameters=dict(self.parameters),
        )

    def to_tool_definition(self) -> ToolDefinition:
        return ToolDefinition(
            name=self.name,
            description=self.description,
            input_schema=dict(self.parameters),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": DOMAIN_TOOL_SCHEMA,
            "name": self.name,
            "domains": list(self.domains),
            "capability": self.capability,
            "description": self.description,
            "schema_digest": self.schema_digest,
            "risk_class": self.risk_class,
            "read_only": self.read_only,
            "approval_required": self.approval_required,
            "execution": "caller_owned_executor",
        }


@dataclass(frozen=True, slots=True)
class AutonomousDomainToolBinding:
    """Explicit application-owned policy for binding one live MCP tool to the brain.

    A binding deliberately contains no executor and no credential material. It is the
    auditable policy layer between a live ``tools/list`` definition and the provider-facing
    domain registry. Tool names, descriptions, and schemas are taken from the authoritative
    catalogue; only domain/capability/effect metadata is supplied by the application.
    """

    name: str
    domains: tuple[str, ...]
    capability: str
    risk_class: str = "read_only"
    read_only: bool = True
    approval_required: bool = False

    def __post_init__(self) -> None:
        name = _identifier("domain tool binding name", self.name)
        domains = _sequence("domain tool binding domains", self.domains, maximum=MAX_DOMAIN_TOOL_DOMAINS)
        capability = _identifier("domain tool binding capability", self.capability)
        if self.risk_class not in DOMAIN_TOOL_RISK_CLASSES:
            raise ArgumentError(
                "domain tool binding risk_class must be one of: " + ", ".join(DOMAIN_TOOL_RISK_CLASSES)
            )
        if not isinstance(self.read_only, bool) or not isinstance(self.approval_required, bool):
            raise ArgumentError("domain tool binding safety flags must be booleans")
        if self.read_only and self.risk_class != "read_only":
            raise ArgumentError("read-only bindings must use risk_class=read_only")
        if not self.read_only and not self.approval_required:
            raise ArgumentError("effectful bindings must require approval")
        object.__setattr__(self, "name", name)
        object.__setattr__(self, "domains", domains)
        object.__setattr__(self, "capability", capability)

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousDomainToolBinding":
        if not isinstance(value, Mapping):
            raise ArgumentError("domain tool binding must be a mapping")
        name = value.get("name", value.get("tool"))
        domains = value.get("domains")
        capability = value.get("capability")
        if not isinstance(name, str):
            raise ArgumentError("domain tool binding requires a string name")
        if not isinstance(domains, Sequence) or isinstance(domains, (str, bytes)):
            raise ArgumentError(f"domain tool binding {name!r} requires domains")
        if not isinstance(capability, str):
            raise ArgumentError(f"domain tool binding {name!r} requires a capability")
        return cls(
            name=name,
            domains=tuple(domains),
            capability=capability,
            risk_class=value.get("risk_class", "read_only"),
            read_only=value.get("read_only", True),
            approval_required=value.get("approval_required", False),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": DOMAIN_TOOL_BINDING_SCHEMA,
            "name": self.name,
            "domains": list(self.domains),
            "capability": self.capability,
            "risk_class": self.risk_class,
            "read_only": self.read_only,
            "approval_required": self.approval_required,
            "authorization": "metadata_only; registration_is_not_authorization",
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class AutonomousDomainToolProfile:
    """Reviewed exact-name recommendations for one autonomous domain.

    Profiles are intentionally metadata-only.  They do not claim that a live MCP tool is
    present, and they never infer safety from a name or description.  The planner below
    intersects these exact names with the live ``tools/list`` snapshot before it proposes
    anything for explicit caller approval.
    """

    domain: str
    description: str
    bindings: tuple[AutonomousDomainToolBinding, ...]

    def __post_init__(self) -> None:
        domain = _identifier("domain tool profile domain", self.domain)
        if domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError(f"unsupported domain tool profile domain: {domain!r}")
        description = _text("domain tool profile description", self.description, maximum=2_000)
        if not isinstance(self.bindings, Sequence) or isinstance(self.bindings, (str, bytes)):
            raise ArgumentError("domain tool profile bindings must be a sequence")
        if not self.bindings or len(self.bindings) > MAX_DOMAIN_TOOLS:
            raise ArgumentError("domain tool profile must contain between 1 and the domain tool limit entries")
        names: set[str] = set()
        normalized: list[AutonomousDomainToolBinding] = []
        for binding in self.bindings:
            if not isinstance(binding, AutonomousDomainToolBinding):
                raise ArgumentError("domain tool profile bindings must be AutonomousDomainToolBinding values")
            if binding.name in names:
                raise ArgumentError(f"domain tool profile contains a duplicate tool: {binding.name}")
            if domain not in binding.domains:
                raise ArgumentError(f"domain tool profile binding {binding.name!r} does not include its profile domain")
            names.add(binding.name)
            normalized.append(binding)
        object.__setattr__(self, "domain", domain)
        object.__setattr__(self, "description", description)
        object.__setattr__(self, "bindings", tuple(normalized))

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": DOMAIN_TOOL_PROFILE_SCHEMA,
            "domain": self.domain,
            "description": self.description,
            "bindings": [binding.to_dict() for binding in self.bindings],
            "execution": "metadata_only; no_live_catalogue_assumption",
        }


def _profile(
    domain: str,
    description: str,
    specs: Sequence[tuple[Any, ...]],
) -> AutonomousDomainToolProfile:
    bindings: list[AutonomousDomainToolBinding] = []
    for spec in specs:
        if len(spec) == 2:
            name, capability = spec
            risk_class, read_only, approval_required = "read_only", True, False
        elif len(spec) == 5:
            name, capability, risk_class, read_only, approval_required = spec
        else:
            raise ArgumentError("domain tool profile specs must contain two or five values")
        bindings.append(
            AutonomousDomainToolBinding(
                name=name,
                domains=(domain,),
                capability=capability,
                risk_class=risk_class,
                read_only=read_only,
                approval_required=approval_required,
            )
        )
    return AutonomousDomainToolProfile(domain, description, tuple(bindings))


def builtin_autonomous_domain_tool_profiles() -> tuple[AutonomousDomainToolProfile, ...]:
    """Return the reviewed exact-name tool recommendations for all built-in domains.

    The profiles are rebuilt from immutable literals so callers cannot mutate shared policy.
    A profile is a recommendation catalogue, not a permission list.  Effectful entries are
    retained as review-only rows and are never emitted in ``proposed_bindings``.
    """

    def review(name: str, capability: str) -> tuple[Any, ...]:
        return name, capability, "external_effect", False, True

    def reversible(name: str, capability: str) -> tuple[Any, ...]:
        return name, capability, "reversible_effect", False, True
    return (
        _profile(
            "coding",
            "Repository inspection, engineering planning, delivery evidence, and release readiness.",
            (
                ("repository_catalog", "repository_inspection"),
                ("repository_bundle", "repository_inspection"),
                ("repository_impact", "repository_impact_analysis"),
                ("developer_platform_status", "platform_observability"),
                ("engineering_manifest_audit", "engineering_contract_audit"),
                ("engineering_execution_plan", "engineering_planning"),
                ("release_pipeline_audit", "release_readiness"),
                ("operational_readiness_audit", "operational_readiness"),
                ("developer_workbench", "developer_workbench"),
                ("developer_workbench_verify", "developer_workbench_verification"),
                ("ci_provider_normalize", "ci_evidence_normalization"),
                ("ci_provider_evidence_audit", "ci_evidence_audit"),
                ("ci_execution_evidence_audit", "ci_execution_audit"),
                ("execution_provenance_audit", "execution_provenance"),
                ("developer_delivery_audit", "delivery_audit"),
                ("developer_delivery_receipt", "delivery_receipt"),
                ("developer_delivery_receipt_verify", "delivery_receipt_verification"),
                ("release_audit", "release_audit"),
                ("sdk_registry_check", "sdk_registry_audit"),
                ("conformance_run", "conformance_verification"),
                ("provider_capability_gate", "provider_capability_verification"),
                ("stewardship_review_check", "stewardship_review"),
                review("agent_mission", "mission_execution"),
            ),
        ),
        _profile(
            "browser",
            "Capability discovery, route inspection, hub lookup, and evidence-source planning.",
            (
                ("workspace_capabilities", "workspace_capability_discovery"),
                ("capability_discover", "capability_discovery"),
                ("capability_route", "capability_routing"),
                ("capability_route_review", "route_review"),
                ("capability_route_plan", "route_planning"),
                ("capability_route_plan_verify", "route_plan_verification"),
                ("hub_search", "hub_discovery"),
                ("hub_resolve", "hub_resolution"),
                ("lens_catalogue", "lens_discovery"),
                ("domain_acquisition_catalogue", "evidence_acquisition_discovery"),
                ("repository_catalog", "repository_inspection"),
                ("domain_evidence_source_plan", "evidence_source_planning"),
                ("domain_evidence_coverage", "evidence_coverage"),
            ),
        ),
        _profile(
            "data",
            "World validation, lineage, structured context compilation, and decision-gated data work.",
            (
                ("world_validate", "world_validation"),
                ("adapter_plan", "data_adapter_planning"),
                ("world_claim_check", "world_claim_validation"),
                ("lineage_audit", "lineage_audit"),
                ("token_context_plan", "context_budget_planning"),
                ("fiber_compile", "context_compilation"),
                ("fiber_refine", "context_refinement"),
                ("fiber_explain", "context_explanation"),
                ("fiber_verify", "context_verification"),
                ("projection_bundle", "projection_bundling"),
                ("obligation_gate_check", "obligation_gate"),
                ("domain_evidence_coverage", "evidence_coverage"),
                ("context_compare", "context_comparison"),
                reversible("tabular_ingest", "tabular_ingestion"),
            ),
        ),
        _profile(
            "science",
            "Literature binding, measurement comparison, inference bounds, laboratory planning, and reproduction.",
            (
                ("literature_bind_check", "literature_binding"),
                ("measurement_compare", "measurement_comparison"),
                ("contradiction_review", "contradiction_review"),
                ("influence_analyze", "influence_analysis"),
                ("lab_plan", "laboratory_planning"),
                ("lab_space_audit", "laboratory_space_audit"),
                ("lab_pareto_audit", "laboratory_pareto_audit"),
                ("lab_branch_audit", "laboratory_branch_audit"),
                ("lab_holdout_audit", "laboratory_holdout_audit"),
                ("lab_evolution_audit", "laboratory_evolution_audit"),
                ("routing_decide", "research_routing"),
                ("routing_lab_run", "research_routing_replay"),
                ("foundation_contract_check", "foundation_contract_validation"),
                ("evaluation_reproduction_check", "reproduction_check"),
                ("epistemic_voi", "value_of_information"),
                ("epistemic_decision_quotient", "decision_quotient"),
                ("epistemic_context_audit", "epistemic_context_audit"),
                ("epistemic_selection_audit", "epistemic_selection_audit"),
                review("epistemic_adaptive_execute", "adaptive_acquisition_execution"),
            ),
        ),
        _profile(
            "biomedical",
            "Biomedical world, modality, safety, ethics, oncology, and evidence-quality boundaries.",
            (
                ("bioworlds_catalog", "biological_world_catalogue"),
                ("world_validate", "world_validation"),
                ("modality_catalog", "modality_catalogue"),
                ("modality_support_check", "modality_support"),
                ("modality_transport_check", "modality_transport"),
                ("modality_comparability_check", "modality_comparability"),
                ("literature_bind_check", "literature_binding"),
                ("measurement_compare", "measurement_comparison"),
                ("contradiction_review", "contradiction_review"),
                ("bioql_compile", "biomedical_query_compilation"),
                ("medical_boundary_check", "medical_boundary"),
                ("bioethics_action_review", "bioethics_action_review"),
                ("bioethics_human_subject_screen", "human_subject_screening"),
                ("bioethics_dual_use_review", "dual_use_review"),
                ("bioethics_validation_check", "bioethics_validation"),
                ("bioethics_representation_audit", "representation_audit"),
                ("bioeval_reference_audit", "biomedical_reference_audit"),
                ("bioeval_grounding_audit", "biomedical_grounding_audit"),
                ("bioeval_estimand_audit", "biomedical_estimand_audit"),
                ("onco_boundary_check", "oncology_boundary"),
                ("onco_response_assess", "oncology_response_assessment"),
                ("onco_worldline_view", "oncology_worldline"),
                ("onco_classification_check", "oncology_classification"),
                ("onco_outcome_analyze", "oncology_outcome_analysis"),
                reversible("world_generate", "biological_world_generation"),
            ),
        ),
        _profile(
            "neuroscience",
            "Neuroimaging modality compatibility, traces, influence bounds, and holdout evaluation.",
            (
                ("modality_catalog", "modality_catalogue"),
                ("modality_support_check", "modality_support"),
                ("modality_transport_check", "modality_transport"),
                ("modality_comparability_check", "modality_comparability"),
                ("measurement_compare", "measurement_comparison"),
                ("trace_analyze", "trajectory_trace_analysis"),
                ("benchmark_trace_analyze", "benchmark_trace_analysis"),
                ("influence_analyze", "influence_analysis"),
                ("lab_holdout_audit", "laboratory_holdout_audit"),
                ("evaluation_trajectory_check", "trajectory_evaluation"),
                ("epistemic_voi", "value_of_information"),
            ),
        ),
        _profile(
            "operations",
            "Operational catalogue, capacity, quality gates, registry posture, release, and runtime evidence.",
            (
                ("operations_catalog", "operations_catalogue"),
                ("ops_acceptance", "operations_acceptance"),
                ("ops_capacity", "capacity_assessment"),
                ("quality_gate_run", "quality_gate"),
                ("telemetry_project", "telemetry_projection"),
                ("registry_gate", "registry_gate"),
                ("registry_lifecycle_simulate", "registry_lifecycle_simulation"),
                ("cache_invalidation_simulate", "cache_invalidation_simulation"),
                ("storage_lifecycle_simulate", "storage_lifecycle_simulation"),
                ("release_audit", "release_audit"),
                ("artifact_registry_audit", "artifact_registry_audit"),
                ("runtime_effect_check", "runtime_effect_check"),
                ("runtime_tape_verify", "runtime_tape_verification"),
                ("operational_readiness_audit", "operational_readiness"),
                ("factory_lifecycle_simulate", "factory_lifecycle_simulation"),
                ("factory_authority_verify", "factory_authority_verification"),
                reversible("ledger_ingest", "ledger_ingestion"),
            ),
        ),
        _profile(
            "enterprise",
            "Governance, security, safety, provider controls, stewardship, and public-hub review.",
            (
                ("policy_screen", "policy_screening"),
                ("safety_posture", "safety_posture"),
                ("security_redteam_simulate", "security_redteam_simulation"),
                ("safety_release_gate", "safety_release_gate"),
                ("medical_boundary_check", "medical_boundary"),
                ("bioethics_dual_use_review", "dual_use_review"),
                ("governance_schema_check", "governance_schema"),
                ("security_privacy_audit", "security_privacy_audit"),
                ("sandbox_admission_audit", "sandbox_admission"),
                ("sandbox_runtime_simulate", "sandbox_runtime_simulation"),
                ("security_program_audit", "security_program_audit"),
                ("provider_capability_gate", "provider_capability_verification"),
                ("stewardship_review_check", "stewardship_review"),
                ("release_audit", "release_audit"),
                ("hub_submission_review", "hub_submission_review"),
                ("hub_disclosure_review", "hub_disclosure_review"),
                review("hub_lock", "hub_lock"),
            ),
        ),
        _profile(
            "multi_agent",
            "Protocol, choreography, workflow, evaluator, and retained mission-evidence coordination.",
            (
                ("weave_protocol_catalog", "protocol_catalogue"),
                ("weavelang_compile", "protocol_compilation"),
                ("choreography_check", "choreography_validation"),
                ("fabric_synthesize", "multi_agent_synthesis"),
                ("interweave_workflow_catalogue", "workflow_catalogue"),
                ("mission_evaluator_discover", "mission_evaluator_discovery"),
                ("mission_evaluator_review", "mission_evaluator_review"),
                ("mission_evaluator_replay", "mission_evaluator_replay"),
                ("mission_evaluator_replay_compare", "mission_evaluator_replay_comparison"),
                ("mission_evidence_bundle_verify", "mission_evidence_verification"),
                ("mission_evidence_bundle_import", "mission_evidence_import"),
                ("mission_evidence_bundle_query", "mission_evidence_query"),
                ("mission_evidence_bundle_get", "mission_evidence_lookup"),
                review("interweave_workflow_execute", "workflow_execution"),
                review("agent_mission", "mission_execution"),
            ),
        ),
        _profile(
            "multimodal",
            "Modality support, transport, comparability, measurement, projection, and presentation surfaces.",
            (
                ("modality_catalog", "modality_catalogue"),
                ("modality_support_check", "modality_support"),
                ("modality_transport_check", "modality_transport"),
                ("modality_comparability_check", "modality_comparability"),
                ("literature_bind_check", "literature_binding"),
                ("measurement_compare", "measurement_comparison"),
                ("projection_bundle", "projection_bundling"),
                ("lens_catalogue", "lens_discovery"),
                ("hub_card_render", "hub_card_rendering"),
                ("context_compare", "context_comparison"),
            ),
        ),
        _profile(
            "cross_domain",
            "Non-executing routing, workflow composition, evidence coverage, and control-plane readiness.",
            (
                ("workspace_capabilities", "workspace_capability_discovery"),
                ("capability_discover", "capability_discovery"),
                ("capability_route", "capability_routing"),
                ("capability_route_review", "route_review"),
                ("capability_route_plan", "route_planning"),
                ("capability_route_plan_verify", "route_plan_verification"),
                ("domain_workflow_catalogue", "workflow_catalogue"),
                ("domain_workflow_scaffold", "workflow_scaffolding"),
                ("domain_workflow_instantiate", "workflow_instantiation"),
                ("domain_workflow_portfolio", "workflow_portfolio"),
                ("domain_workflow_portfolio_verify", "workflow_portfolio_verification"),
                ("domain_workflow_verify", "workflow_verification"),
                ("domain_evidence_intake", "evidence_intake"),
                ("domain_evidence_coverage", "evidence_coverage"),
                ("domain_evidence_source_plan", "evidence_source_planning"),
                ("control_plane_readiness_audit", "control_plane_readiness"),
                ("provider_normalize", "provider_normalization"),
                ("provider_replay", "provider_replay"),
                review("domain_evidence_source_execute", "evidence_source_execution"),
            ),
        ),
        _profile(
            "evaluation",
            "Benchmark, oracle, adaptive-panel, reproduction, integrity, and research-CI evaluation.",
            (
                ("context_compare", "context_comparison"),
                ("prism_minimize", "evaluation_minimization"),
                ("adaptive_panel", "adaptive_evaluation_panel"),
                ("posterior_gate", "posterior_gate"),
                ("evaluation_worldline_audit", "worldline_evaluation"),
                ("evaluation_reproduction_check", "reproduction_check"),
                ("evaluation_trajectory_check", "trajectory_evaluation"),
                ("benchmark_trace_analyze", "benchmark_trace_analysis"),
                ("benchmark_decision_audit", "benchmark_decision_audit"),
                ("benchmark_integrity_audit", "benchmark_integrity_audit"),
                ("benchmark_counterfactual_check", "benchmark_counterfactual"),
                ("benchmark_oracle_review", "benchmark_oracle_review"),
                ("benchmark_compile", "benchmark_compilation"),
                ("benchmark_compile_review", "benchmark_compilation_review"),
                ("oracle_combine", "oracle_combination"),
                ("oracle_reference_panel", "oracle_reference_panel"),
                ("oracle_missingness", "oracle_missingness"),
                ("research_ci_check", "research_ci"),
                ("metrics_profile_audit", "metrics_profile_audit"),
                ("metrics_analytics_audit", "metrics_analytics_audit"),
                ("bioeval_reference_audit", "biomedical_reference_audit"),
                ("bioeval_grounding_audit", "biomedical_grounding_audit"),
                review("epistemic_adaptive_execute", "adaptive_acquisition_execution"),
            ),
        ),
    )


def _profile_binding_index(
    profiles: Sequence[AutonomousDomainToolProfile],
) -> dict[str, AutonomousDomainToolBinding]:
    """Merge shared exact-name profile rows while rejecting policy disagreement."""

    merged: dict[str, AutonomousDomainToolBinding] = {}
    for profile in profiles:
        for binding in profile.bindings:
            previous = merged.get(binding.name)
            if previous is None:
                merged[binding.name] = binding
                continue
            if (
                previous.capability != binding.capability
                or previous.risk_class != binding.risk_class
                or previous.read_only != binding.read_only
                or previous.approval_required != binding.approval_required
            ):
                raise ArgumentError(f"built-in domain tool profiles disagree about {binding.name!r}")
            merged[binding.name] = AutonomousDomainToolBinding(
                name=binding.name,
                domains=tuple(sorted(set(previous.domains).union(binding.domains))),
                capability=binding.capability,
                risk_class=binding.risk_class,
                read_only=binding.read_only,
                approval_required=binding.approval_required,
            )
    return merged


def plan_mcp_catalogue_bindings(
    catalogue: ToolCatalogue | Sequence[Mapping[str, Any] | ToolDefinition],
    *,
    domains: Sequence[str] | None = None,
) -> dict[str, Any]:
    """Create a deterministic, non-mutating binding plan from a live MCP catalogue.

    Only exact names in the reviewed profiles can become proposals.  A live tool absent from
    those profiles is reported as unclassified, while a known effectful row is reported for
    review and excluded from the automatically applicable ``proposed_bindings`` mapping.
    """

    snapshot = catalogue if isinstance(catalogue, ToolCatalogue) else ToolCatalogue.from_definitions(catalogue)
    selected_domains = tuple(AUTONOMOUS_DOMAIN_NAMES) if domains is None else _sequence(
        "domain tool binding plan domains", domains, maximum=len(AUTONOMOUS_DOMAIN_NAMES)
    )
    unknown_domains = sorted(set(selected_domains).difference(AUTONOMOUS_DOMAIN_NAMES))
    if unknown_domains:
        raise ArgumentError("domain tool binding plan contains unknown domains: " + ", ".join(unknown_domains))
    all_profiles = builtin_autonomous_domain_tool_profiles()
    profile_map = {profile.domain: profile for profile in all_profiles}
    selected_profiles = tuple(profile_map[domain] for domain in selected_domains)
    index = _profile_binding_index(selected_profiles)
    global_index = _profile_binding_index(all_profiles)
    definitions = {definition.name: definition for definition in snapshot.definitions}

    def binding_row(binding: AutonomousDomainToolBinding) -> dict[str, Any]:
        row = binding.to_dict()
        row["live_schema_digest"] = definitions[binding.name].schema_digest
        row["catalogue_digest"] = snapshot.digest
        return row

    proposed = {
        name: binding_row(binding)
        for name, binding in sorted(index.items())
        if name in definitions and binding.read_only
    }
    review_required = {
        name: binding_row(binding)
        for name, binding in sorted(index.items())
        if name in definitions and not binding.read_only
    }
    live_names = set(definitions)
    selected_names = set(index)
    coverage: dict[str, dict[str, Any]] = {}
    for profile in selected_profiles:
        required = {binding.name: binding for binding in profile.bindings}
        available_names = sorted(set(required).intersection(definitions))
        proposed_names = sorted(set(available_names).intersection(proposed))
        required_capabilities = sorted({binding.capability for binding in required.values()})
        available_capabilities = sorted({required[name].capability for name in available_names})
        proposed_capabilities = sorted({required[name].capability for name in proposed_names})
        coverage[profile.domain] = {
            "profile_schema": DOMAIN_TOOL_PROFILE_SCHEMA,
            "required_tool_count": len(required),
            "available_tool_count": len(available_names),
            "proposed_tool_count": len(proposed_names),
            "available_tools": available_names,
            "proposed_tools": proposed_names,
            "missing_tools": sorted(set(required).difference(available_names)),
            "required_capabilities": required_capabilities,
            "available_capabilities": available_capabilities,
            "proposed_capabilities": proposed_capabilities,
            "missing_capabilities": sorted(set(required_capabilities).difference(available_capabilities)),
            "coverage_ratio": round(len(available_names) / len(required), 6) if required else 1.0,
            "approved_coverage_ratio": round(len(proposed_names) / len(required), 6) if required else 1.0,
        }

    plan = {
        "schema": DOMAIN_TOOL_BINDING_PLAN_SCHEMA,
        "catalogue_digest": snapshot.digest,
        "catalogue_tool_count": len(snapshot.definitions),
        "profile_digest": content_digest([profile.to_dict() for profile in selected_profiles]),
        "profile_catalogue_digest": content_digest([profile.to_dict() for profile in all_profiles]),
        "domains": list(selected_domains),
        "domain_count": len(selected_domains),
        "available_curated_tools": sorted(live_names.intersection(selected_names)),
        "missing_curated_tools": sorted(selected_names.difference(live_names)),
        "unclassified_tools": sorted(live_names.difference(set(global_index))),
        "review_required_tools": sorted(review_required),
        "proposed_bindings": proposed,
        "review_required_bindings": review_required,
        "coverage": coverage,
        "review_required": True,
        "authorization": "metadata_only; planning_does_not_register_or_authorize_tools",
        "execution": "planning_only; no_registry_mutation; no_tool_execution",
        "policy": {
            "matching": "exact_curated_tool_name_only",
            "schemas": "live_catalogue_is_authoritative",
            "unknown_tools": "never_inferred_as_safe",
            "effectful_tools": "manual_review_and_separate_approval_required",
            "credentials": "never_included",
        },
    }
    return _json_safe("domain tool binding plan", plan, maximum=MAX_DOMAIN_TOOL_BINDING_PLAN_BYTES)


class AutonomousDomainToolRegistry:
    """Bounded, duplicate-free registry of tools available to the autonomous brain."""

    def __init__(self, tools: Sequence[AutonomousDomainTool] = ()) -> None:
        if not isinstance(tools, Sequence) or isinstance(tools, (str, bytes)):
            raise ArgumentError("domain tools must be a sequence")
        if len(tools) > MAX_DOMAIN_TOOLS:
            raise ArgumentError(f"domain tools may contain at most {MAX_DOMAIN_TOOLS} entries")
        self._tools: dict[str, AutonomousDomainTool] = {}
        for tool in tools:
            self.register(tool)

    def register(self, tool: AutonomousDomainTool, *, replace_existing: bool = False) -> AutonomousDomainTool:
        if not isinstance(tool, AutonomousDomainTool):
            raise ArgumentError("domain tool registry entries must be AutonomousDomainTool values")
        if tool.name in self._tools and not replace_existing:
            raise ArgumentError(f"domain tool is already registered: {tool.name}")
        if len(self._tools) >= MAX_DOMAIN_TOOLS and tool.name not in self._tools:
            raise ArgumentError(f"domain tools may contain at most {MAX_DOMAIN_TOOLS} entries")
        self._tools[tool.name] = tool
        return tool

    def register_mcp_definition(
        self,
        definition: Mapping[str, Any] | ToolDefinition,
        *,
        domains: Sequence[str],
        capability: str,
        risk_class: str = "read_only",
        read_only: bool = True,
        approval_required: bool = False,
        replace_existing: bool = False,
    ) -> AutonomousDomainTool:
        return self.register(
            AutonomousDomainTool.from_mcp_definition(
                definition,
                domains=domains,
                capability=capability,
                risk_class=risk_class,
                read_only=read_only,
                approval_required=approval_required,
            ),
            replace_existing=replace_existing,
        )

    def register_mcp_catalogue(
        self,
        catalogue: ToolCatalogue | Sequence[Mapping[str, Any] | ToolDefinition],
        bindings: Mapping[str, AutonomousDomainToolBinding | Mapping[str, Any]],
        *,
        require_all: bool = True,
        replace_existing: bool = False,
    ) -> tuple[AutonomousDomainTool, ...]:
        """Atomically bind a live MCP catalogue using explicit application policy.

        ``tools/list`` definitions are never interpreted as permissions. Every registered
        definition must have a caller-supplied binding unless ``require_all=False``; unknown
        binding names are rejected so a typo cannot silently leave a tool ungoverned. All
        definitions and bindings are validated before this registry is mutated.
        """

        snapshot = catalogue if isinstance(catalogue, ToolCatalogue) else ToolCatalogue.from_definitions(catalogue)
        if not isinstance(bindings, Mapping):
            raise ArgumentError("domain tool bindings must be a mapping keyed by tool name")
        definitions = {definition.name: definition for definition in snapshot.definitions}
        normalized: dict[str, AutonomousDomainToolBinding] = {}
        for key, value in bindings.items():
            if not isinstance(key, str) or not key.strip():
                raise ArgumentError("domain tool binding keys must be non-empty strings")
            if isinstance(value, AutonomousDomainToolBinding):
                binding = value
            else:
                if not isinstance(value, Mapping):
                    raise ArgumentError(f"domain tool binding {key!r} must be a mapping")
                raw_binding = dict(value)
                raw_binding.setdefault("name", key)
                binding = AutonomousDomainToolBinding.from_mapping(raw_binding)
            if binding.name != key:
                raise ArgumentError(
                    f"domain tool binding key {key!r} does not match binding name {binding.name!r}"
                )
            normalized[key] = binding
        unknown = sorted(set(normalized).difference(definitions))
        if unknown:
            raise ArgumentError("bindings reference tools absent from the live catalogue: " + ", ".join(unknown))
        missing = sorted(set(definitions).difference(normalized))
        if require_all and missing:
            raise ArgumentError("live catalogue tools are missing explicit bindings: " + ", ".join(missing))
        selected = [
            AutonomousDomainTool.from_mcp_definition(
                definition,
                domains=normalized[definition.name].domains,
                capability=normalized[definition.name].capability,
                risk_class=normalized[definition.name].risk_class,
                read_only=normalized[definition.name].read_only,
                approval_required=normalized[definition.name].approval_required,
            )
            for definition in snapshot.definitions
            if definition.name in normalized
        ]
        selected_names = {tool.name for tool in selected}
        existing_conflicts = sorted(selected_names.intersection(self._tools))
        if existing_conflicts and not replace_existing:
            raise ArgumentError("domain tools are already registered: " + ", ".join(existing_conflicts))
        if len(self._tools) - len(existing_conflicts) + len(selected) > MAX_DOMAIN_TOOLS:
            raise ArgumentError(f"domain tools may contain at most {MAX_DOMAIN_TOOLS} entries")
        for tool in selected:
            self._tools[tool.name] = tool
        return tuple(selected)

    def plan_mcp_catalogue_bindings(
        self,
        catalogue: ToolCatalogue | Sequence[Mapping[str, Any] | ToolDefinition],
        *,
        domains: Sequence[str] | None = None,
    ) -> dict[str, Any]:
        """Plan exact curated bindings without reading or mutating this registry."""

        return plan_mcp_catalogue_bindings(catalogue, domains=domains)

    def resolve(self, name: str) -> AutonomousDomainTool:
        _identifier("domain tool name", name)
        tool = self._tools.get(name)
        if tool is None:
            raise ToolSchemaError(f"domain tool {name!r} is not registered")
        return tool

    def tools_for(
        self,
        domains: Sequence[str] | None = None,
        *,
        capabilities: Sequence[str] = (),
        include_shared: bool = True,
    ) -> tuple[AutonomousDomainTool, ...]:
        if domains is not None:
            domain_set = set(_sequence("tool selection domains", domains, maximum=MAX_DOMAIN_TOOL_DOMAINS))
        else:
            domain_set = None
        capability_set = set(_sequence("tool selection capabilities", capabilities, maximum=MAX_DOMAIN_TOOLS)) if capabilities else set()
        selected: list[AutonomousDomainTool] = []
        for tool in self._tools.values():
            if domain_set is not None:
                matches = bool(domain_set.intersection(tool.domains))
                if include_shared and "cross_domain" in tool.domains:
                    matches = True
                if not matches:
                    continue
            if capability_set and tool.capability not in capability_set:
                continue
            selected.append(tool)
        return tuple(sorted(selected, key=lambda item: item.name))

    def provider_tools(
        self,
        domains: Sequence[str] | None = None,
        *,
        capabilities: Sequence[str] = (),
    ) -> tuple[ProviderTool, ...]:
        return tuple(tool.to_provider_tool() for tool in self.tools_for(domains, capabilities=capabilities))

    def catalogue(self, domains: Sequence[str] | None = None) -> list[dict[str, Any]]:
        return [tool.to_dict() for tool in self.tools_for(domains)]

    @property
    def digest(self) -> str:
        return content_digest([tool.to_dict() for tool in self.tools_for()])

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": DOMAIN_TOOL_REGISTRY_SCHEMA,
            "digest": self.digest,
            "tool_count": len(self._tools),
            "tools": self.catalogue(),
            "execution": "metadata_only",
        }


@dataclass(frozen=True, slots=True)
class AutonomousDomainToolReceipt:
    """Metadata-only receipt for one tool intent handled by the domain runtime."""

    call_id: str
    tool: str
    status: str
    schema_digest: str | None = None
    arguments_digest: str | None = None
    output_digest: str | None = None
    execution_id: str | None = None
    domain: str | None = None
    capability: str | None = None
    risk_class: str | None = None

    def __post_init__(self) -> None:
        _text("tool receipt call_id", self.call_id, maximum=256)
        _identifier("tool receipt tool", self.tool)
        if self.status not in DOMAIN_TOOL_EXECUTION_STATUSES:
            raise ArgumentError("tool receipt status is invalid")
        for name, value in (("schema_digest", self.schema_digest), ("arguments_digest", self.arguments_digest), ("output_digest", self.output_digest)):
            if value is not None and (not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value)):
                raise ArgumentError(f"{name} must be a lowercase SHA-256 digest or None")
        for name, value in (("execution_id", self.execution_id), ("domain", self.domain), ("capability", self.capability), ("risk_class", self.risk_class)):
            if value is not None:
                _identifier(f"tool receipt {name}", value)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": DOMAIN_TOOL_SCHEMA,
            "call_id": self.call_id,
            "tool": self.tool,
            "status": self.status,
            "schema_digest": self.schema_digest,
            "arguments_digest": self.arguments_digest,
            "output_digest": self.output_digest,
            "execution_id": self.execution_id,
            "domain": self.domain,
            "capability": self.capability,
            "risk_class": self.risk_class,
            "retention": "metadata_only_no_arguments_or_outputs",
        }


class AutonomousDomainToolRuntime:
    """Approval-aware adapter from provider intents to a caller-owned tool executor."""

    def __init__(
        self,
        registry: AutonomousDomainToolRegistry,
        *,
        executor: Callable[[AutonomousDomainTool, Mapping[str, Any]], Any],
        approve: Callable[[AutonomousDomainTool, ProviderToolCall], bool] | None = None,
        auto_execute_read_only: bool = True,
        controller: AutonomousExecutionController | None = None,
        _receipt_store: list[AutonomousDomainToolReceipt] | None = None,
        _scope: tuple[str, str] | None = None,
    ) -> None:
        if not isinstance(registry, AutonomousDomainToolRegistry):
            raise ArgumentError("domain tool runtime requires an AutonomousDomainToolRegistry")
        if not callable(executor):
            raise ArgumentError("domain tool runtime executor must be callable")
        if approve is not None and not callable(approve):
            raise ArgumentError("domain tool runtime approval callback must be callable")
        if not isinstance(auto_execute_read_only, bool):
            raise ArgumentError("auto_execute_read_only must be a boolean")
        if controller is not None and not isinstance(controller, AutonomousExecutionController):
            raise ArgumentError("domain tool runtime controller must be an AutonomousExecutionController or None")
        if _receipt_store is not None and not isinstance(_receipt_store, list):
            raise ArgumentError("domain tool runtime receipt store must be a list or None")
        if _scope is not None:
            if (
                not isinstance(_scope, tuple)
                or len(_scope) != 2
                or not all(isinstance(value, str) for value in _scope)
            ):
                raise ArgumentError("domain tool runtime scope must contain execution_id and domain")
            _text("domain tool runtime scope execution_id", _scope[0], maximum=256)
            _identifier("domain tool runtime scope domain", _scope[1])
        self.registry = registry
        self.executor = executor
        self.approve = approve
        self.auto_execute_read_only = auto_execute_read_only
        self.controller = controller
        self._receipts = _receipt_store if _receipt_store is not None else []
        self._scope = _scope

    def scoped(self, *, execution_id: str, domain: str) -> "AutonomousDomainToolRuntime":
        """Create a non-persistent run scope that still binds receipt identity to a domain."""

        resolved_execution_id = (
            self.controller.state.execution_id
            if self.controller is not None
            else _text("domain tool scope execution_id", execution_id, maximum=256)
        )
        return AutonomousDomainToolRuntime(
            self.registry,
            executor=self.executor,
            approve=self.approve,
            auto_execute_read_only=self.auto_execute_read_only,
            controller=self.controller,
            _receipt_store=self._receipts,
            _scope=(
                resolved_execution_id,
                _identifier("domain tool scope domain", domain),
            ),
        )

    def session(
        self,
        *,
        execution_id: str,
        domain: str,
        capability: str,
        risk_class: str,
        policy: AutonomousExecutionPolicy | Mapping[str, Any] | None = None,
        journal: AutonomousExecutionJournal | None = None,
        resume: bool = False,
    ) -> "AutonomousDomainToolRuntime":
        """Create an isolated run controller while preserving the application receipt stream."""

        controller = AutonomousExecutionController(
            execution_id=execution_id,
            domain=domain,
            capability=capability,
            risk_class=risk_class,
            policy=policy,
            journal=journal,
            resume=resume,
        )
        return AutonomousDomainToolRuntime(
            self.registry,
            executor=self.executor,
            approve=self.approve,
            auto_execute_read_only=self.auto_execute_read_only,
            controller=controller,
            _receipt_store=self._receipts,
        )

    @property
    def receipts(self) -> tuple[AutonomousDomainToolReceipt, ...]:
        return tuple(self._receipts)

    def _result(
        self,
        call: ProviderToolCall,
        *,
        status: str,
        content: Mapping[str, Any],
        approved: bool,
        is_error: bool = True,
        receipt: AutonomousDomainToolReceipt,
    ) -> ProviderToolResult:
        self._receipts.append(receipt)
        return ProviderToolResult(call.call_id, dict(content), approved=approved, is_error=is_error)

    def __call__(self, calls: tuple[ProviderToolCall, ...]) -> tuple[ProviderToolResult, ...]:
        if not isinstance(calls, tuple) or not calls or len(calls) > MAX_DOMAIN_TOOL_CALLS:
            raise ArgumentError("domain tool runtime received an invalid call batch")
        if any(not isinstance(call, ProviderToolCall) for call in calls):
            raise ArgumentError("domain tool runtime received malformed provider calls")

        scoped_execution_id = None if self._scope is None else self._scope[0]
        scoped_domain = None if self._scope is None else self._scope[1]
        execution_id = self.controller.state.execution_id if self.controller is not None else scoped_execution_id
        domain = scoped_domain if self._scope is not None else (
            None if self.controller is None else self.controller.state.domain
        )
        if len({call.call_id for call in calls}) != len(calls):
            return tuple(
                self._result(
                    call,
                    status="schema_refused",
                    content={"status": "refused", "reason": "duplicate_call_ids", "authorization": "approval_required"},
                    approved=False,
                    receipt=AutonomousDomainToolReceipt(
                        call.call_id,
                        call.name,
                        "schema_refused",
                        execution_id=execution_id,
                        domain=domain,
                    ),
                )
                for call in calls
            )

        prepared: list[tuple[ProviderToolCall, AutonomousDomainTool, dict[str, Any], str]] = []
        refusals: list[tuple[ProviderToolCall, str, AutonomousDomainToolReceipt]] = []
        for call in calls:
            arguments_digest = content_digest(dict(call.arguments))
            try:
                _reject_secret_fields(dict(call.arguments))
                tool = self.registry.resolve(call.name)
                plan = ToolCatalogue.from_definitions([tool.to_tool_definition()]).plan(call.name, call.arguments)
            except (ArgumentError, ToolSchemaError):
                refusals.append(
                    (
                        call,
                        "schema_refused",
                        AutonomousDomainToolReceipt(
                            call.call_id,
                            call.name,
                            "schema_refused",
                            arguments_digest=arguments_digest,
                            execution_id=execution_id,
                            domain=domain,
                        ),
                    )
                )
                continue
            if self.controller is not None:
                try:
                    self.controller.admit_tool_call(
                        tool=tool.name,
                        call_id=call.call_id,
                        read_only=tool.read_only,
                        approval_required=tool.approval_required,
                    )
                except AutonomyPolicyError:
                    refusals.append(
                        (
                            call,
                            "policy_refused",
                            AutonomousDomainToolReceipt(
                                call.call_id,
                                tool.name,
                                "policy_refused",
                                schema_digest=plan.schema_digest,
                                arguments_digest=arguments_digest,
                                execution_id=execution_id,
                                domain=domain,
                                capability=tool.capability,
                                risk_class=tool.risk_class,
                            ),
                        )
                    )
                    continue
            authorized = tool.read_only and self.auto_execute_read_only
            if not authorized and self.approve is not None:
                try:
                    authorized = bool(self.approve(tool, call))
                except Exception:
                    authorized = False
            if not authorized:
                refusals.append(
                    (
                        call,
                        "approval_required",
                        AutonomousDomainToolReceipt(
                            call.call_id,
                            tool.name,
                            "approval_required",
                            schema_digest=plan.schema_digest,
                            arguments_digest=arguments_digest,
                            execution_id=execution_id,
                            domain=domain,
                            capability=tool.capability,
                            risk_class=tool.risk_class,
                        ),
                    )
                )
                continue
            prepared.append((call, tool, plan.arguments, arguments_digest))

        if refusals:
            results: list[ProviderToolResult] = []
            for call in calls:
                matching = next((item for item in refusals if item[0].call_id == call.call_id), None)
                if matching is not None:
                    _call, status, receipt = matching
                    results.append(
                        self._result(
                            call,
                            status=status,
                            content={"status": "refused", "tool": call.name, "reason": status, "authorization": "approval_required"},
                            approved=False,
                            receipt=receipt,
                        )
                    )
                    if self.controller is not None and status == "approval_required":
                        tool = self.registry.resolve(call.name)
                        self.controller.record_tool_outcome(
                            tool=tool.name,
                            call_id=call.call_id,
                            status="approval_required",
                            reason="caller_approval_missing",
                        )
                else:
                    tool = next(item[1] for item in prepared if item[0].call_id == call.call_id)
                    results.append(
                        self._result(
                            call,
                            status="approval_required",
                            content={"status": "refused", "tool": tool.name, "reason": "batch_contains_unapproved_call", "authorization": "approval_required"},
                            approved=False,
                            receipt=AutonomousDomainToolReceipt(
                                call.call_id,
                                tool.name,
                                "approval_required",
                                schema_digest=tool.schema_digest,
                                arguments_digest=content_digest(dict(call.arguments)),
                                execution_id=execution_id,
                                domain=domain,
                                capability=tool.capability,
                                risk_class=tool.risk_class,
                            ),
                        )
                    )
                    if self.controller is not None:
                        self.controller.record_tool_outcome(
                            tool=tool.name,
                            call_id=call.call_id,
                            status="approval_required",
                            reason="batch_contains_unapproved_call",
                        )
            return tuple(results)

        results: list[ProviderToolResult] = []
        for call, tool, arguments, arguments_digest in prepared:
            try:
                output = self.executor(tool, arguments)
                _json_safe("domain tool result", output, maximum=MAX_DOMAIN_TOOL_RESULT_BYTES)
                _reject_secret_fields(output)
                results.append(
                    self._result(
                        call,
                        status="executed",
                        content=output if isinstance(output, Mapping) else {"result": output},
                        approved=True,
                        is_error=False,
                        receipt=AutonomousDomainToolReceipt(
                            call.call_id,
                            tool.name,
                            "executed",
                            schema_digest=tool.schema_digest,
                            arguments_digest=arguments_digest,
                            output_digest=content_digest(output if isinstance(output, Mapping) else {"result": output}),
                            execution_id=execution_id,
                            domain=domain,
                            capability=tool.capability,
                            risk_class=tool.risk_class,
                        ),
                    )
                )
                if self.controller is not None:
                    self.controller.record_tool_outcome(
                        tool=tool.name,
                        call_id=call.call_id,
                        status="executed",
                        outcome_digest=content_digest(output if isinstance(output, Mapping) else {"result": output}),
                    )
            except Exception:
                results.append(
                    self._result(
                        call,
                        status="execution_failed",
                        content={"status": "execution_failed", "tool": tool.name, "authorization": "caller_approved"},
                        approved=True,
                        receipt=AutonomousDomainToolReceipt(
                            call.call_id,
                            tool.name,
                            "execution_failed",
                            schema_digest=tool.schema_digest,
                            arguments_digest=arguments_digest,
                            execution_id=execution_id,
                            domain=domain,
                            capability=tool.capability,
                            risk_class=tool.risk_class,
                        ),
                    )
                )
                if self.controller is not None:
                    self.controller.record_tool_outcome(
                        tool=tool.name,
                        call_id=call.call_id,
                        status="execution_failed",
                        reason="executor_error",
                    )
        return tuple(results)


__all__ = [
    "AUTONOMOUS_DOMAIN_NAMES",
    "DOMAIN_TOOL_BINDING_SCHEMA",
    "DOMAIN_TOOL_BINDING_PLAN_SCHEMA",
    "DOMAIN_TOOL_EXECUTION_STATUSES",
    "DOMAIN_TOOL_PROFILE_SCHEMA",
    "DOMAIN_TOOL_REGISTRY_SCHEMA",
    "DOMAIN_TOOL_RISK_CLASSES",
    "DOMAIN_TOOL_SCHEMA",
    "MAX_DOMAIN_TOOL_BINDING_PLAN_BYTES",
    "MAX_DOMAIN_TOOL_CALLS",
    "MAX_DOMAIN_TOOLS",
    "AutonomousDomainTool",
    "AutonomousDomainToolBinding",
    "AutonomousDomainToolProfile",
    "AutonomousDomainToolReceipt",
    "AutonomousDomainToolRegistry",
    "AutonomousDomainToolRuntime",
    "builtin_autonomous_domain_tool_profiles",
    "plan_mcp_catalogue_bindings",
]
