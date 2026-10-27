"""Fixed reviewed capability profiles for operator-selected workflow catalogue extensions.

This application extension has no blueprint module identifier. Profiles name concrete reviewed
code paths, never request-supplied import paths, credentials or execution grants. A profile may
contain several readers and local operations; neither its count nor descriptor availability is
a verified service count. Existing catalogue descriptors and executor bindings remain intact.
"""
from __future__ import annotations

from collections.abc import Sequence
from typing import Any

from .authoring import canonical_json
from .errors import ArgumentError
from .workflow_integrations import WorkflowOperationCatalog, builtin_workflow_operation_catalog

WORKFLOW_CAPABILITIES_SCHEMA = "aurora-workflow-capabilities/0.1"
MAX_WORKFLOW_CAPABILITY_SELECTIONS = 32
WORKFLOW_CAPABILITY_IDS = ("gene-identity", "glioma-ontology")
_PROFILES = {
    "gene-identity": {
        "label": "TP53 gene identifier evidence review",
        "scope": "Fixed public TP53 HGNC/Ensembl/NCBI metadata; source agreement is not validated biological equivalence.",
        "required_operations": ["hgnc.gene_references", "ensembl.gene_references", "ncbi_gene.summary",
                                "glioma.gene_identity.analyze", "glioma.gene_identity.review_handoff"],
    },
    "glioma-ontology": {
        "label": "NCI glioma terminology and relationships",
        "scope": "Fixed NCIt Glioma concept and requested direct relationships; terminology is not cohort or clinical evidence.",
        "required_operations": ["nci_thesaurus.glioma_ontology"],
    },
}


def validate_workflow_capabilities(profile_ids: Sequence[str]) -> tuple[str, ...]:
    """Validate every selection before loading an extension or mutating application state."""
    if (isinstance(profile_ids, (str, bytes)) or not isinstance(profile_ids, Sequence)
            or len(profile_ids) > MAX_WORKFLOW_CAPABILITY_SELECTIONS
            or any(type(name) is not str or name not in WORKFLOW_CAPABILITY_IDS for name in profile_ids)
            or len(set(profile_ids)) != len(profile_ids)):
        raise ArgumentError("workflow capabilities require unique reviewed profile IDs within their bound")
    return tuple(sorted(profile_ids))


def _extension(profile_id: str) -> WorkflowOperationCatalog:
    catalog = WorkflowOperationCatalog()
    if profile_id == "gene-identity":
        from .reviewed_hgnc_gene_retrieval import register_hgnc_gene_operations
        from .reviewed_ensembl_gene_retrieval import register_ensembl_gene_operations
        from .glioma_gene_identity_workflow import register_glioma_gene_identity_workflow_operations
        catalog = builtin_workflow_operation_catalog().subset(["ncbi_gene.summary"])
        register_hgnc_gene_operations(catalog)
        register_ensembl_gene_operations(catalog)
        register_glioma_gene_identity_workflow_operations(catalog)
    elif profile_id == "glioma-ontology":
        from .reviewed_nci_thesaurus_retrieval import register_nci_thesaurus_operations
        register_nci_thesaurus_operations(catalog)
    else:
        raise ArgumentError("workflow capability is not reviewed")
    if {item.operation_id for item in catalog.operations()} != set(_PROFILES[profile_id]["required_operations"]):
        raise ArgumentError("workflow capability code and reviewed operation inventory disagree")
    return catalog


def with_workflow_capabilities(catalog: WorkflowOperationCatalog, profile_ids: Sequence[str]) -> WorkflowOperationCatalog:
    """Return an extended catalogue without replacing existing matching executors or inputs."""
    if type(catalog) is not WorkflowOperationCatalog:
        raise ArgumentError("workflow capabilities require a concrete operation catalogue")
    selected = validate_workflow_capabilities(profile_ids)
    extended = catalog
    for profile_id in selected:
        extended = extended.extended(_extension(profile_id))
    return extended


def workflow_capability_inventory(catalog: WorkflowOperationCatalog, profile_ids: Sequence[str]) -> dict[str, Any]:
    """Inspect explicit operator selections and descriptor presence, without dispatch."""
    if type(catalog) is not WorkflowOperationCatalog:
        raise ArgumentError("workflow capabilities require a concrete operation catalogue")
    selected = validate_workflow_capabilities(profile_ids)
    installed = {item.operation_id: item.to_dict() for item in catalog.operations()}
    profiles = []
    for profile_id in WORKFLOW_CAPABILITY_IDS:
        description = _PROFILES[profile_id]
        enabled = profile_id in selected
        missing, mismatched = [], []
        if enabled:
            for operation in _extension(profile_id).operations():
                current = installed.get(operation.operation_id)
                if current is None:
                    missing.append(operation.operation_id)
                elif canonical_json(current) != canonical_json(operation.to_dict()):
                    mismatched.append(operation.operation_id)
        profiles.append({"profile_id": profile_id, **{key: list(value) if type(value) is list else value for key, value in description.items()},
            "operator_enabled": enabled, "descriptor_available": enabled and not missing and not mismatched,
            "missing_operations": missing, "mismatched_operations": mismatched,
            "executor_conformance": "not_established_by_profile_presence", "source_availability": "unmeasured"})
    return {"schema": WORKFLOW_CAPABILITIES_SCHEMA, "selected_profiles": list(selected), "profiles": profiles,
            "execution_authority": "none;model_and_source_grants_remain_separate",
            "inventory_unit": "reviewed_application_profile;not_a_provider_count"}


def compile_workflow_capability(task: str, catalog: WorkflowOperationCatalog, profile_id: str, *,
                                enabled_profiles: Sequence[str], requirements: Any = None) -> Any:
    """Compile only an operator-enabled fixed source composition, without a model or source call."""
    selected = validate_workflow_capabilities(enabled_profiles)
    if type(profile_id) is not str or profile_id not in selected:
        raise ArgumentError("workflow capability is not enabled by this operator")
    inventory = workflow_capability_inventory(catalog, selected)
    if not next(item for item in inventory["profiles"] if item["profile_id"] == profile_id)["descriptor_available"]:
        raise ArgumentError("workflow capability descriptors are unavailable or changed")
    if profile_id == "gene-identity":
        from .glioma_gene_identity_workflow import compile_glioma_gene_identity_workflow
        return compile_glioma_gene_identity_workflow(task, catalog, requirements=requirements)
    from .workflow_builder import compile_workflow_plan
    from .workflow_requirements import compile_workflow_requirements, merge_workflow_requirements
    operation = "nci_thesaurus.glioma_ontology"
    arguments = {"timeout_ms": 30_000}
    native = compile_workflow_requirements(task, required_operations=[operation], output_operations=[operation],
        max_steps=1, max_calls_by_operation={operation: 1},
        argument_rules=[{"operation": operation, "path": "", "comparison": "equals", "value": arguments}])
    if requirements is not None:
        native = merge_workflow_requirements(task, native, requirements, catalog=catalog)
    return compile_workflow_plan({"name": "NCI glioma source terminology inspection", "steps": [
        {"id": "ontology", "operation": operation, "arguments": arguments, "depends_on": []}],
        "outputs": ["ontology"]}, catalog, task=task, requirements=native)


__all__ = ["WORKFLOW_CAPABILITIES_SCHEMA", "MAX_WORKFLOW_CAPABILITY_SELECTIONS", "WORKFLOW_CAPABILITY_IDS",
           "validate_workflow_capabilities", "with_workflow_capabilities", "workflow_capability_inventory", "compile_workflow_capability"]
