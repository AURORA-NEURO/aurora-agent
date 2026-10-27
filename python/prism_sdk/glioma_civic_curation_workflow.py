"""Local review of native CIViC curation metadata, without scientific interpretation.

Repository-defined extension: no available blueprint module specifies this composition.
Every projection replays from the existing bounded raw source bundle. Curation categories,
flags, revision counts and repeated source IDs never establish quality or independence.
"""
from __future__ import annotations

from collections.abc import Mapping
import json
from typing import Any

from .authoring import canonical_json, content_digest
from .domain_tools import _reject_secret_fields
from .errors import ArgumentError
from .reviewed_civic_evidence_retrieval import (
    CIVIC_EVIDENCE_OPERATION_ID, MAX_CIVIC_EVIDENCE_ITEMS, validate_civic_evidence_bundle,
)

GLIOMA_CIVIC_CURATION_SCOPE = "public_glioblastoma_name_CIViC_curation_metadata_review"
GLIOMA_CIVIC_CURATION_ANALYSIS_SCHEMA = "aurora-glioma-civic-curation-analysis/0.1"
GLIOMA_CIVIC_CURATION_REVIEW_SCHEMA = "aurora-glioma-civic-curation-review/0.1"
GLIOMA_CIVIC_CURATION_ANALYSIS_OPERATION = "glioma.civic_curation.analyze"
GLIOMA_CIVIC_CURATION_REVIEW_OPERATION = "glioma.civic_curation.review_handoff"
MAX_GLIOMA_CIVIC_CURATION_BYTES = 8_000_000
MAX_GLIOMA_CIVIC_CURATION_CITATIONS = 512
# At most six connection/page citations and, per item, one row, ten item fields,
# three disease fields and three source fields. Missing fields only reduce this count.
_NATIVE_CITATION_BOUND = 6 + 17 * MAX_CIVIC_EVIDENCE_ITEMS
_LIMITATIONS = [
    "The question labels one fixed accepted glioblastoma-name first page; it does not supply a source query or establish arbitrary task completeness.",
    "CIViC status, type, level, direction, rating, flags and revision counts remain source curation assertions, not scientific quality, confidence or clinical support.",
    "Every supplied item remains separate. Grouping identical native source IDs establishes only repeated IDs, not publication identity, independence, patients, cohorts or assays.",
    "Same-ID differing citation labels and different IDs with matching labels remain unresolved native metadata, without normalization or a preferred publication.",
    "Missing source, an observed empty page, missing fields, explicit nulls and observed values remain distinct; none supplies negative scientific evidence or an imputed score.",
    "Source coverage, later-page assertions, missingness, excluded terms/statuses/fields and original limitations remain intact with unknown completeness and omitted influence.",
    "Native citations cover this supplied window only. No local citation omission does not establish source exhaustiveness, scientific sufficiency or zero influence of excluded evidence.",
    "Raw-byte replay and self-computed hashes establish internal consistency, not authenticated source truth, live acquisition, human review or scientific validity.",
    "No descriptions, therapies, molecular profiles, variants, sequences, patient records, source abstracts, retraction assessment, clinical or laboratory actions are added.",
]
_REVIEW_ACTIONS = [
    "Inspect every native item, curation category/direction/rating, flag and revision count without translating these into scientific quality.",
    "Inspect raw/native provenance, missing/null fields and repeated native source IDs with all asserted citation labels; do not merge publications or discard items.",
    "Review the fixed disease-name/status/first-page window, inert pagination metadata and all original omissions and source limitations.",
    "Obtain separately authorized independent publication and scientific review before any scientific support, clinical inference, population/assay join or denominator claim.",
]


def _safe(value: Any) -> Any:
    try:
        text = canonical_json(dict(value) if isinstance(value, Mapping) else value)
        if len(text.encode("utf-8")) > MAX_GLIOMA_CIVIC_CURATION_BYTES:
            raise ArgumentError("CIViC curation artifact exceeds its whole-artifact byte bound")
        result = json.loads(text)
        _reject_secret_fields(result)
        return result
    except (ValueError, TypeError, UnicodeError, RecursionError, OverflowError):
        raise ArgumentError("CIViC curation requires bounded canonical JSON") from None


def _question(value: Any) -> str:
    try:
        valid = type(value) is str and bool(value.strip()) and "\x00" not in value and len(value.encode("utf-8")) <= 8000
    except UnicodeError:
        valid = False
    if not valid:
        raise ArgumentError("CIViC curation question must be bounded nonempty text")
    return value


def _pointer(value: Any, pointer: str) -> Any:
    for token in pointer.split("/")[1:]:
        token = token.replace("~1", "/").replace("~0", "~")
        value = value[int(token)] if type(value) is list else value[token]
    return value


def _citation(bundle: dict[str, Any], pointer: str, kind: str) -> dict[str, Any]:
    source = bundle["source_inputs"][0]
    value = _pointer(source["native_payload"], pointer)
    unsigned = {"source_operation_id": CIVIC_EVIDENCE_OPERATION_ID, "record_kind": kind,
        "source_bundle_digest": bundle["bundle_digest"], "source_response_digest": source["response_digest"],
        "source_uri": source["url"], "retrieved_at": bundle["retrieved_at"],
        "artifact_pointer_root": "validated_civic_bundle",
        "artifact_pointer": "/source_inputs/0/native_payload" + pointer, "native_payload_pointer": pointer,
        "source_value": value, "source_value_digest": content_digest(value)}
    return {"citation_id": "glioma_civic_curation:" + content_digest(unsigned), **unsigned}


def analyze_glioma_civic_curation(*, question: str, civic_bundle: Mapping[str, Any] | None = None) -> dict[str, Any]:
    """Retain all curation assertions and native provenance; never rank their scientific value."""
    question = _question(question)
    bundle = validate_civic_evidence_bundle(_safe(civic_bundle)) if civic_bundle is not None else None
    citations: list[dict[str, Any]] = []
    items: list[dict[str, Any]] = []
    groups: dict[int, dict[str, Any]] = {}
    connection_citations: dict[str, Any] | None = None

    def cite(pointer: str, kind: str) -> str:
        if len(citations) >= MAX_GLIOMA_CIVIC_CURATION_CITATIONS:
            raise ArgumentError("CIViC curation refuses citation overflow instead of truncating")
        citation = _citation(bundle, pointer, kind)
        citations.append(citation)
        return citation["citation_id"]

    def links(observations: dict[str, Any], context: str, kind: str) -> dict[str, Any]:
        return {field: {"observation_state": observation["state"],
                       "value_citation_id": None if observation["native_pointer"] is None else cite(observation["native_pointer"], kind + field),
                       "absence_context_citation_id": context if observation["state"] == "missing" else None}
                for field, observation in observations.items()}

    if bundle is not None:
        root = "/data/evidenceItems"
        page = bundle["source_inputs"][0]["native_payload"]["data"]["evidenceItems"]["pageInfo"]
        connection_citations = {"connection": cite(root, "native_connection"),
            "totalCount": cite(root + "/totalCount", "native_filtered_total_count"),
            "pageInfo": cite(root + "/pageInfo", "native_page_info"),
            "hasNextPage": cite(root + "/pageInfo/hasNextPage", "native_has_next_page"),
            "endCursor": {"value_citation_id": cite(root + "/pageInfo/endCursor", "native_end_cursor") if "endCursor" in page else None,
                          "absence_context_citation_id": None},
            "nodes": cite(root + "/nodes", "native_evidence_item_array")}
        if "endCursor" not in page:
            connection_citations["endCursor"]["absence_context_citation_id"] = connection_citations["pageInfo"]
        for index, record in enumerate(bundle["records"]):
            row_citation = cite(record["native_pointer"], "native_evidence_item")
            field_links = links(record["observations"], row_citation, "native_item_")
            disease_links = (None if record["disease_fields"] is None else
                             links(record["disease_fields"], field_links["disease"]["value_citation_id"], "native_disease_"))
            source_links = links(record["source_fields"], field_links["source"]["value_citation_id"], "native_source_")
            observations = record["observations"]
            attention = []
            if observations["flagged"]["value"] is True:
                attention.append("source_flagged")
            if observations["openRevisionCount"]["value"] > 0:
                attention.append("source_open_revisions_reported")
            for field in ("disease", "evidenceRating"):
                if observations[field]["state"] != "observed":
                    attention.append("source_" + field + "_" + observations[field]["state"])
            if record["disease_fields"] is not None and record["disease_fields"]["doid"]["state"] != "observed":
                attention.append("source_doid_" + record["disease_fields"]["doid"]["state"])
            items.append({"record_index": index, "evidence_id": record["evidence_id"],
                "native_pointer": record["native_pointer"], "record_citation_id": row_citation,
                "observations": observations, "observation_citations": field_links,
                "disease_fields": record["disease_fields"], "disease_field_citations": disease_links,
                "source_fields": record["source_fields"], "source_field_citations": source_links,
                "source_citation": record["citation"], "metadata_attention_reasons": attention,
                "attention_is_scientific_quality_ranking": False})
            native_source = observations["source"]["value"]
            identity = native_source["id"]
            if identity not in groups:
                groups[identity] = {"native_source_id": identity, "members": []}
            groups[identity]["members"].append({"record_index": index, "evidence_id": record["evidence_id"],
                "native_source": native_source, "source_field_citations": source_links,
                "record_citation_id": row_citation})
        for group in groups.values():
            labels = {canonical_json({key: member["native_source"][key] for key in ("citationId", "displayType")})
                      for member in group["members"]}
            group["source_metadata_state"] = ("same_native_id_differing_metadata" if len(labels) > 1 else
                                               "same_native_id_consistent_metadata")
            group["member_evidence_item_count"] = len(group["members"])
            group["publication_identity_or_independence"] = "unestablished"
    if len(citations) > _NATIVE_CITATION_BOUND:
        raise ArgumentError("CIViC curation native citation contract changed")
    value = {"schema": GLIOMA_CIVIC_CURATION_ANALYSIS_SCHEMA, "scope": GLIOMA_CIVIC_CURATION_SCOPE,
        "question": question, "question_used_as_source_query": False,
        "status": "needs_evidence" if bundle is None else "curation_metadata_available_for_review",
        "source_state": ("not_supplied" if bundle is None else "observed_empty_window" if not items else "observed_nonempty_window"),
        "source_inputs": {"civic_bundle": bundle}, "item_observations": items, "native_source_id_groups": list(groups.values()),
        "grouping_policy": {"key": "exact_native_source_id_integer", "ordering": "first_native_item_occurrence",
            "label_normalization_performed": False, "evidence_items_deduplicated": False,
            "observed_native_source_id_group_count": None if bundle is None else len(groups),
            "count_unit": "distinct_observed_native_source_ids_in_supplied_page;not_independent_publications",
            "source_independence": "unassessed"},
        "source_coverage": None if bundle is None else bundle["coverage"],
        "source_omissions": None if bundle is None else bundle["omissions"],
        "source_limitations": None if bundle is None else bundle["limitations"],
        "source_origin_authentication": "unestablished", "connection_citations": connection_citations,
        "citation_index": citations, "citation_policy": {"selection": "all_derived_native_citations_no_ranking",
            "max_citations": MAX_GLIOMA_CIVIC_CURATION_CITATIONS, "native_window_maximum": _NATIVE_CITATION_BOUND,
            "count": len(citations), "omitted_local_citation_count": 0,
            "omission_scope": "derived_index_only;source_omissions_and_influence_unchanged"},
        "scientific_support_established": False, "scientific_support": "unestablished", "clinical_support": "unestablished",
        "independent_publication_count": None, "patient_or_cohort_or_assay_denominator": None,
        "cohorts_joined": False, "measurements_joined": False, "human_review_required": True,
        "clinical_actions_authorized": False, "limitations": list(_LIMITATIONS)}
    return _safe({**value, "analysis_digest": content_digest(value)})


def validate_glioma_civic_curation_analysis(value: Mapping[str, Any]) -> dict[str, Any]:
    actual = _safe(value)
    try:
        if set(actual["source_inputs"]) != {"civic_bundle"}:
            raise ArgumentError("CIViC curation replay source fields changed")
        expected = analyze_glioma_civic_curation(question=actual["question"], civic_bundle=actual["source_inputs"]["civic_bundle"])
    except (KeyError, TypeError):
        raise ArgumentError("CIViC curation analysis lacks native replay inputs") from None
    if canonical_json(actual) != canonical_json(expected):
        raise ArgumentError("CIViC curation analysis does not replay from native sources")
    return expected


def glioma_civic_curation_review_handoff(analysis: Mapping[str, Any]) -> dict[str, Any]:
    checked = validate_glioma_civic_curation_analysis(analysis)
    value = {"schema": GLIOMA_CIVIC_CURATION_REVIEW_SCHEMA, "scope": GLIOMA_CIVIC_CURATION_SCOPE,
        "status": "needs_evidence" if checked["source_state"] == "not_supplied" else "awaiting_curation_metadata_research_review",
        "analysis": checked, "analysis_digest": checked["analysis_digest"], "citation_index": checked["citation_index"],
        "human_review_required": True, "scientific_support_established": False,
        "scientific_support": "unestablished", "clinical_support": "unestablished",
        "independent_publication_count": None, "patient_or_cohort_or_assay_denominator": None,
        "clinical_actions_authorized": False, "review_actions": list(_REVIEW_ACTIONS), "limitations": list(_LIMITATIONS)}
    return _safe({**value, "handoff_digest": content_digest(value)})


def validate_glioma_civic_curation_handoff(value: Mapping[str, Any]) -> dict[str, Any]:
    actual = _safe(value)
    if type(actual) is not dict or "analysis" not in actual:
        raise ArgumentError("CIViC curation handoff lacks native replay analysis")
    expected = glioma_civic_curation_review_handoff(actual["analysis"])
    if canonical_json(actual) != canonical_json(expected):
        raise ArgumentError("CIViC curation handoff does not replay")
    return expected


def register_glioma_civic_curation_workflow_operations(catalog: Any) -> Any:
    from .workflow_integrations import (MAX_WORKFLOW_INTEGRATIONS, MAX_WORKFLOW_OPERATIONS,
        WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult)
    if not isinstance(catalog, WorkflowOperationCatalog):
        raise ArgumentError("CIViC curation requires a workflow catalogue")
    operations = catalog.operations()
    if {row.operation_id for row in operations} & {GLIOMA_CIVIC_CURATION_ANALYSIS_OPERATION, GLIOMA_CIVIC_CURATION_REVIEW_OPERATION}:
        raise ArgumentError("CIViC curation operations already registered")
    integration = "local.glioma_civic_curation"
    if (len(operations) + 2 > MAX_WORKFLOW_OPERATIONS or integration not in {row.integration_id for row in operations}
            and len({row.integration_id for row in operations}) >= MAX_WORKFLOW_INTEGRATIONS):
        raise ArgumentError("CIViC curation registration exceeds catalogue capacity")
    def analyze(arguments: Mapping[str, Any], **_kwargs: Any) -> Any:
        value = analyze_glioma_civic_curation(**dict(arguments))
        return WorkflowOperationResult(value, {"operation_id": GLIOMA_CIVIC_CURATION_ANALYSIS_OPERATION,
            "analysis_digest": value["analysis_digest"], "effect": "local_read_only"})
    def review(arguments: Mapping[str, Any], **_kwargs: Any) -> Any:
        value = glioma_civic_curation_review_handoff(arguments["analysis"])
        return WorkflowOperationResult(value, {"operation_id": GLIOMA_CIVIC_CURATION_REVIEW_OPERATION,
            "handoff_digest": value["handoff_digest"], "effect": "local_read_only"})
    catalog.register(WorkflowOperation(GLIOMA_CIVIC_CURATION_ANALYSIS_OPERATION,
        "Replay every native CIViC curation item, flag, revision, omission and identical source ID for research metadata review; no scientific quality, publication independence or denominator inference.",
        {"type": "object", "properties": {"question": {"type": "string", "minLength": 1, "maxLength": 8000},
            "civic_bundle": {"type": ["object", "null"]}}, "required": ["question", "civic_bundle"], "additionalProperties": False},
        {"type": "object", "required": ["schema", "scope", "analysis_digest", "source_inputs", "citation_index"],
            "properties": {"schema": {"const": GLIOMA_CIVIC_CURATION_ANALYSIS_SCHEMA}, "scope": {"const": GLIOMA_CIVIC_CURATION_SCOPE},
                "analysis_digest": {"type": "string"}, "source_inputs": {"type": "object"}, "citation_index": {"type": "array"}}},
        integration, implementation="local"), analyze)
    catalog.register(WorkflowOperation(GLIOMA_CIVIC_CURATION_REVIEW_OPERATION,
        "Replay native CIViC curation metadata into an explicit research review hold; source levels/directions/ratings are not scientific support or clinical confidence.",
        {"type": "object", "properties": {"analysis": {"type": "object"}}, "required": ["analysis"], "additionalProperties": False},
        {"type": "object", "required": ["schema", "scope", "handoff_digest", "analysis", "status"],
            "properties": {"schema": {"const": GLIOMA_CIVIC_CURATION_REVIEW_SCHEMA}, "scope": {"const": GLIOMA_CIVIC_CURATION_SCOPE},
                "handoff_digest": {"type": "string"}, "analysis": {"type": "object"}, "status": {"enum": ["needs_evidence", "awaiting_curation_metadata_research_review"]}}},
        integration, implementation="local"), review)
    return catalog


def glioma_civic_curation_workflow_proposal(question: str) -> dict[str, Any]:
    question = _question(question)
    return {"name": "CIViC glioblastoma-name curation metadata research review", "steps": [
        {"id": "civic", "operation": CIVIC_EVIDENCE_OPERATION_ID,
            "arguments": {"timeout_ms": 30_000}, "depends_on": []},
        {"id": "analysis", "operation": GLIOMA_CIVIC_CURATION_ANALYSIS_OPERATION,
            "arguments": {"question": question, "civic_bundle": {"$ref": "civic#"}}, "depends_on": ["civic"]},
        {"id": "review", "operation": GLIOMA_CIVIC_CURATION_REVIEW_OPERATION,
            "arguments": {"analysis": {"$ref": "analysis#"}}, "depends_on": ["analysis"]}], "outputs": ["review"]}


def compile_glioma_civic_curation_workflow(task: str, catalog: Any, requirements: Any = None) -> Any:
    """Bind the one source call and both exact native-root links without acquiring evidence."""
    from .workflow_builder import compile_workflow_plan
    from .workflow_requirements import compile_workflow_requirements, merge_workflow_requirements
    proposal = glioma_civic_curation_workflow_proposal(task)
    operations = [step["operation"] for step in proposal["steps"]]
    native = compile_workflow_requirements(task, required_operations=operations,
        output_operations=[GLIOMA_CIVIC_CURATION_REVIEW_OPERATION], max_steps=3,
        max_calls_by_operation={operation: 1 for operation in operations},
        argument_rules=[{"operation": CIVIC_EVIDENCE_OPERATION_ID, "path": "", "comparison": "equals", "value": {"timeout_ms": 30_000}},
                        {"operation": GLIOMA_CIVIC_CURATION_ANALYSIS_OPERATION, "path": "/question", "comparison": "equals", "value": task}],
        data_links=[{"source_operation": CIVIC_EVIDENCE_OPERATION_ID, "target_operation": GLIOMA_CIVIC_CURATION_ANALYSIS_OPERATION,
                     "source_pointer": "", "target_path": "/civic_bundle"},
                    {"source_operation": GLIOMA_CIVIC_CURATION_ANALYSIS_OPERATION, "target_operation": GLIOMA_CIVIC_CURATION_REVIEW_OPERATION,
                     "source_pointer": "", "target_path": "/analysis"}])
    if requirements is not None:
        native = merge_workflow_requirements(task, native, requirements, catalog=catalog)
    return compile_workflow_plan(proposal, catalog, task=task, requirements=native)
