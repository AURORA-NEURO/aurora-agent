"""The one Aurora capability registry shared by every provider profile.

``AURORA_CAPABILITIES`` is a static snapshot of MCP tool names taken from the
``bioprism-mcp`` launcher contract (the tool list printed by
``crates/mcp/src/main.rs --help``). It exists so allowlists fail closed on
typos without contacting a server; it is deliberately *not* the live registry.
The authoritative, current catalogue is what the server itself advertises via
``tools/list`` and the ``bioprism://capabilities/0.1`` resource. A name absent
here may exist upstream; a name present here is guaranteed to have been in the
contract when this snapshot was recorded — both facts are asserted against the
launcher source by ``tests/test_registry.py``.
"""

from __future__ import annotations

from .errors import UnknownCapabilityError

#: Registry snapshot provenance, exposed so diagnostics can cite it.
REGISTRY_SNAPSHOT_SOURCE = "crates/mcp/src/main.rs --help tool list"

_AURORA_CAPABILITY_NAMES: tuple[str, ...] = (
    "adaptive_panel",
    "atlas_report",
    "benchmark_trace_analyze",
    "bioeval_reference_audit",
    "bioworlds_catalog",
    "bioql_compile",
    "bundle_verify",
    "brain_plan",
    "cache_invalidation_simulate",
    "choreography_check",
    "conformance_run",
    "context_compare",
    "contradiction_review",
    "epistemic_voi",
    "evaluation_reproduction_check",
    "fiber_compile",
    "fiber_explain",
    "fiber_refine",
    "fiber_verify",
    "hub_lock",
    "hub_resolve",
    "hub_search",
    "influence_analyze",
    "interweave_workflow_catalogue",
    "lab_plan",
    "lens_catalogue",
    "ledger_ingest",
    "lineage_audit",
    "measurement_compare",
    "medical_boundary_check",
    "modality_catalog",
    "mutation_family",
    "onco_boundary_check",
    "oracle_combine",
    "pack_catalogue",
    "pack_health_assess",
    "policy_screen",
    "posterior_gate",
    "prism_minimize",
    "projection_bundle",
    "quality_gate_run",
    "registry_gate",
    "registry_lifecycle_simulate",
    "release_audit",
    "repository_catalog",
    "resource_workbench_discover",
    "routing_decide",
    "safety_release_gate",
    "storage_lifecycle_simulate",
    "stress_profile",
    "stress_report",
    "token_context_plan",
    "trace_analyze",
    "trace_otel_ingest",
    "weavelang_compile",
    "world_generate",
    "world_index",
    "world_validate",
    "workspace_capabilities",
)

AURORA_CAPABILITIES: frozenset[str] = frozenset(_AURORA_CAPABILITY_NAMES)


def validate_allowlist(allowlist: object) -> tuple[str, ...]:
    """Validate an allowlist against the registry and return it canonically sorted.

    ``None`` means unrestricted and is preserved as ``None`` by callers; an
    explicit allowlist must be non-empty (an empty one is ambiguous between
    "everything" and "nothing", and ambiguity in a security boundary is how
    refusals become answers).
    """
    if allowlist is None:
        return ()
    if isinstance(allowlist, str) or not isinstance(allowlist, (list, tuple, frozenset, set)):
        raise UnknownCapabilityError("allowlist must be None or a sequence of capability names")
    names = list(allowlist)
    if not names:
        raise UnknownCapabilityError("allowlist must be None (unrestricted) or non-empty")
    for name in names:
        if not isinstance(name, str):
            raise UnknownCapabilityError("allowlist entries must be strings")
    unknown = sorted(set(names) - AURORA_CAPABILITIES)
    if unknown:
        raise UnknownCapabilityError(
            f"unknown Aurora capabilities: {', '.join(unknown)} "
            f"(registry snapshot: {REGISTRY_SNAPSHOT_SOURCE}; live source: tools/list)"
        )
    if len(set(names)) != len(names):
        duplicates = sorted({name for name in names if names.count(name) > 1})
        raise UnknownCapabilityError(f"allowlist repeats capabilities: {', '.join(duplicates)}")
    return tuple(sorted(names))
