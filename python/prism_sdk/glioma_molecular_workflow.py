"""Executable public glioma molecular catalogue and association review composition.

This repository-defined adapter replays the existing reviewed source normalizers. It does not
coerce their bundles into RealGliomaBundle, join Entrez and Ensembl identifiers, infer assay
results from profile availability, or implement causal/statistical inference. Native source
receipts remain in the workflow journal; this local layer verifies self-computed bundle/source
integrity and derives a replayable metadata inventory, not source authentication.
"""
from __future__ import annotations

from collections import Counter
from collections.abc import Mapping, Sequence
import json
import math
from typing import Any

from .authoring import canonical_json, content_digest
from .domain_tools import _reject_secret_fields
from .errors import ArgumentError
from . import reviewed_cbioportal_study_retrieval as cbio
from . import reviewed_ncbi_gene_retrieval as gene
from . import reviewed_open_targets_retrieval as targets
from . import reviewed_gwas_catalog_retrieval as gwas

GLIOMA_MOLECULAR_SCOPE = "public_glioma_molecular_metadata"
GLIOMA_MOLECULAR_COVERAGE_SCHEMA = "aurora-glioma-molecular-coverage/0.1"
GLIOMA_MOLECULAR_REVIEW_SCHEMA = "aurora-glioma-molecular-review/0.1"
GLIOMA_MOLECULAR_COVERAGE_OPERATION = "glioma.molecular.coverage"
GLIOMA_MOLECULAR_REVIEW_OPERATION = "glioma.molecular.review_handoff"
MAX_MOLECULAR_SOURCE_BYTES = 750_000
MAX_MOLECULAR_ARTIFACT_BYTES = 2_000_000
_OWNERS = {"cbioportal.studies": cbio, "ncbi_gene.summary": gene, "open_targets.associations": targets, "gwas_catalog.associations": gwas}
_SCHEMAS = {"cbioportal.studies": cbio.REVIEWED_CBIOPORTAL_BUNDLE_SCHEMA, "ncbi_gene.summary": gene.REVIEWED_NCBI_GENE_BUNDLE_SCHEMA, "open_targets.associations": targets.REVIEWED_OPEN_TARGETS_BUNDLE_SCHEMA, "gwas_catalog.associations": gwas.REVIEWED_GWAS_CATALOG_BUNDLE_SCHEMA}
_LIMITATIONS = [
    "Fixed reviewed source selectors are used; the question does not refine their queries.",
    "Study/profile catalogues describe availability, not measured mutation, expression, methylation, or patient outcomes.",
    "Entrez, Ensembl, gene symbols, and GWAS mapped-gene annotations remain distinct; no crosswalk or alias matching is performed.",
    "Association scores are source rankings, not confidence or causality; GWAS p-values and mapped genes do not establish mechanisms or treatment relevance.",
    "Absence from a bounded page is unknown, not negative biological evidence; source and local citation omissions remain explicit.",
    "Cross-source cohort overlap, evidence independence, ancestry applicability, and scientific validity are unassessed; counts are not pooled across sources.",
    "Self-computed digests establish internal consistency, not authenticated source truth or clinical authority.",
    "Reviewed source adapters retrieve no patient-level records; no diagnosis, treatment, laboratory action, or routing into the preclinical engine is supplied.",
]


def _safe(value: Any, maximum: int = MAX_MOLECULAR_ARTIFACT_BYTES) -> Any:
    try:
        encoded = canonical_json(value)
        if len(encoded.encode("utf-8")) > maximum:
            raise ArgumentError("glioma molecular metadata exceeds its byte bound")
        result = json.loads(encoded)
        _reject_secret_fields(result)
        return result
    except (TypeError, ValueError, RecursionError, UnicodeError) as error:
        if isinstance(error, ArgumentError):
            raise
        raise ArgumentError("glioma molecular metadata must be bounded canonical JSON") from error


def _integer(name: str, value: Any, minimum: int, maximum: int) -> int:
    if type(value) is not int or not minimum <= value <= maximum:
        raise ArgumentError(f"{name} is outside its integer bound")
    return value


def _question(value: Any) -> str:
    try:
        valid = isinstance(value, str) and bool(value.strip()) and "\x00" not in value and len(value.encode("utf-8")) <= 8000
    except UnicodeError:
        valid = False
    if not valid:
        raise ArgumentError("molecular research question must be bounded nonempty text")
    return value


def _exact(value: Any, keys: set[str], name: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        raise ArgumentError(f"{name} has fields outside its native public metadata contract")
    return value


def _same(actual: Any, expected: Any, name: str) -> None:
    if canonical_json(actual) != canonical_json(expected):
        raise ArgumentError(f"{name} does not replay from the native reviewed source contract")


def _source(module: Any, source_id: str, uri: str, timestamp: str, payload: Any, count: int, **extra: Any) -> dict[str, Any]:
    authority = {cbio: cbio.REVIEWED_CBIOPORTAL_AUTHORITY, gene: gene.REVIEWED_NCBI_GENE_AUTHORITY, targets: targets.REVIEWED_OPEN_TARGETS_AUTHORITY, gwas: gwas.REVIEWED_GWAS_CATALOG_AUTHORITY}[module]
    return {"source_id": source_id, "authority": authority, "uri": uri, "retrieved_at": timestamp, "content_sha256": content_digest(payload), "record_count": count, "provider": "none", "credentials": "none" if module is gene else "not_accepted", "limitations": list(module._LIMITATIONS), **extra}


def _replay_cbio(bundle: dict[str, Any]) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    _exact(bundle, {"schema", "generated_at", "sources", "studies", "molecular_profiles", "source_set_digest", "study_count", "profile_count", "completeness", "provider", "credentials", "limitations", "bundle_digest"}, "cBioPortal bundle")
    studies, profiles = bundle["studies"], bundle["molecular_profiles"]
    if not isinstance(studies, list) or not 1 <= len(studies) <= 2 or not isinstance(profiles, list) or len(profiles) > 254:
        raise ArgumentError("cBioPortal catalogue exceeds its reviewed bounds")
    study_ids = [row.get("study_id") for row in studies if isinstance(row, dict)]
    allowed = list(cbio.REVIEWED_CBIOPORTAL_STUDIES.values())
    if len(study_ids) != len(studies) or any(study_id not in allowed for study_id in study_ids) or study_ids != [item for item in allowed if item in study_ids]:
        raise ArgumentError("cBioPortal study selection is invalid or duplicated")
    expected_sources, expected_profiles, records, coverage = [], [], [], []
    for study in studies:
        study_id = study["study_id"]
        normalized = cbio._study({"studyId": study_id, "name": study.get("name"), "description": study.get("description"), "cancerTypeId": study.get("cancer_type_id"), "publicStudy": study.get("public_study"), "pmid": study.get("pmid"), "allSampleCount": study.get("sample_count"), "referenceGenome": study.get("reference_genome"), "importDate": study.get("import_date")}, study_id)
        _same(study, normalized, "cBioPortal study")
        selected = [row for row in profiles if isinstance(row, dict) and row.get("study_id") == study_id]
        normalized_profiles = cbio._profiles([{"studyId": study_id, "molecularProfileId": row.get("profile_id"), "name": row.get("name"), "molecularAlterationType": row.get("molecular_alteration_type"), "genericAssayType": row.get("generic_assay_type"), "datatype": row.get("datatype"), "description": row.get("description"), "showProfileInAnalysisTab": row.get("show_in_analysis"), "patientLevel": row.get("patient_level")} for row in selected], study_id)
        if len(normalized_profiles) >= cbio.MAX_REVIEWED_CBIOPORTAL_PROFILES_PER_STUDY:
            raise ArgumentError("cBioPortal catalogue completeness is unresolved at the source page limit")
        _same(selected, normalized_profiles, "cBioPortal molecular profiles")
        expected_profiles.extend(normalized_profiles)
        payload = {"study": normalized, "molecular_profiles": normalized_profiles}
        expected_sources.append(_source(cbio, normalized["source_id"], cbio._study_url(study_id), bundle["generated_at"], payload, 1 + len(normalized_profiles), profile_uri=cbio._profiles_url(study_id)))
        records.append({"kind": "study_catalogue", "record_id": study_id, "source_id": normalized["source_id"], "uri": f"https://www.cbioportal.org/study/summary?id={study_id}", "data": normalized})
        for profile in normalized_profiles:
            records.append({"kind": "molecular_profile_catalogue", "record_id": profile["profile_id"], "source_id": normalized["source_id"], "uri": cbio._profiles_url(study_id), "data": profile})
        coverage.append({"study_id": study_id, "source_id": normalized["source_id"], "source_reported_sample_count": normalized["sample_count"], "profile_count": len(normalized_profiles), "alteration_type_counts": dict(sorted(Counter(row["molecular_alteration_type"] for row in normalized_profiles).items())), "datatype_counts": dict(sorted(Counter(row["datatype"] for row in normalized_profiles).items())), "missing_description_count": sum(row["description"] is None for row in normalized_profiles), "unknown_patient_level_flag_count": sum(row["patient_level"] is None for row in normalized_profiles), "measurement_values": "not_retrieved"})
    _same(profiles, expected_profiles, "cBioPortal complete profile partition")
    _same(bundle["sources"], expected_sources, "cBioPortal provenance")
    _same(bundle["study_count"], len(studies), "cBioPortal study count")
    _same(bundle["profile_count"], len(profiles), "cBioPortal profile count")
    _same(bundle["completeness"], "complete" if all(row["metadata_completeness"] == "complete" for row in studies) and profiles else "unknown", "cBioPortal completeness")
    return records, {"studies": coverage}


def _replay_gene(bundle: dict[str, Any]) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    _exact(bundle, {"schema", "generated_at", "sources", "genes", "source_set_digest", "gene_count", "catalogue_coverage", "provider", "credentials", "limitations", "bundle_digest"}, "NCBI Gene bundle")
    genes = bundle["genes"]
    if not isinstance(genes, list) or not 1 <= len(genes) <= 12:
        raise ArgumentError("NCBI Gene identity rows exceed their reviewed bounds")
    symbols = [row.get("symbol") for row in genes if isinstance(row, dict)]
    if len(symbols) != len(genes) or symbols != [symbol for symbol in gene.REVIEWED_NCBI_GENE_CATALOGUE if symbol in symbols]:
        raise ArgumentError("NCBI Gene symbol selection is invalid or duplicated")
    normalized = []
    for row in genes:
        symbol = row["symbol"]
        gene_id = gene.REVIEWED_NCBI_GENE_CATALOGUE[symbol]
        if not isinstance(row.get("aliases"), list) or any(not isinstance(alias, str) for alias in row["aliases"]):
            raise ArgumentError("NCBI Gene aliases must retain their native string list")
        payload = {"result": {"uids": [gene_id], gene_id: {"uid": gene_id, "name": symbol, "description": row.get("description"), "chromosome": row.get("chromosome"), "maplocation": row.get("map_location"), "otheraliases": ",".join(row["aliases"]), "organism": {"taxid": row.get("organism_taxid")}}}}
        replayed = gene._normalise_gene(payload, symbol)
        _same(row, replayed, "NCBI Gene identity")
        normalized.append(replayed)
    ids = [gene.REVIEWED_NCBI_GENE_CATALOGUE[symbol] for symbol in symbols]
    _same(bundle["sources"], [_source(gene, "ncbi_gene", f"https://www.ncbi.nlm.nih.gov/gene/?term={','.join(ids)}", bundle["generated_at"], normalized, len(genes))], "NCBI Gene provenance")
    _same(bundle["gene_count"], len(genes), "NCBI Gene count")
    _same(bundle["catalogue_coverage"], "complete", "NCBI Gene requested catalogue coverage")
    return [{"kind": "gene_identity", "record_id": row["gene_id"], "source_id": "ncbi_gene", "uri": f"https://www.ncbi.nlm.nih.gov/gene/{row['gene_id']}", "data": row} for row in genes], {"requested_gene_count": len(genes), "fixed_catalogue_size": 12, "unrequested_catalogue_symbols": [symbol for symbol in gene.REVIEWED_NCBI_GENE_CATALOGUE if symbol not in symbols], "identity_namespace": "NCBI_Entrez_Gene", "functional_or_variant_interpretation": "not_supplied"}


def _replay_targets(bundle: dict[str, Any]) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    _exact(bundle, {"schema", "generated_at", "sources", "associations", "source_set_digest", "association_count", "coverage", "provider", "credentials", "limitations", "bundle_digest"}, "Open Targets bundle")
    rows = bundle["associations"]
    if not isinstance(rows, list) or not 1 <= len(rows) <= 2:
        raise ArgumentError("Open Targets disease lanes exceed their reviewed bounds")
    lanes = [row.get("lane") for row in rows if isinstance(row, dict)]
    if len(lanes) != len(rows) or lanes != [lane for lane in targets.REVIEWED_OPEN_TARGETS_LANES if lane in lanes]:
        raise ArgumentError("Open Targets disease selection is invalid or duplicated")
    records, sources, summaries = [], [], []
    for row in rows:
        page_size = _integer("Open Targets page size", row.get("page_size"), 1, 50)
        target_rows = row.get("targets")
        if not isinstance(target_rows, list) or len(target_rows) > 50:
            raise ArgumentError("Open Targets target rows exceed their page bound")
        payload = {"data": {"disease": {"id": row.get("disease_id"), "name": row.get("disease_name"), "associatedTargets": {"count": row.get("total_associations"), "rows": [{"target": {"id": item.get("target_id"), "approvedSymbol": item.get("approved_symbol")}, "score": item.get("association_score")} for item in target_rows if isinstance(item, dict)]}}}}
        normalized = targets._association(payload, row["lane"], page_size)
        _same(row, normalized, "Open Targets ranked lane")
        sources.append(_source(targets, row["source_id"], targets.REVIEWED_OPEN_TARGETS_ENDPOINT, bundle["generated_at"], row, row["returned_associations"]))
        summaries.append({key: row[key] for key in ("lane", "disease_id", "disease_name", "total_associations", "returned_associations", "omitted_associations", "coverage", "score_interpretation")})
        for target in row["targets"]:
            records.append({"kind": "ranked_gene_association", "record_id": row["disease_id"] + ":" + target["target_id"], "source_id": row["source_id"], "uri": f"https://platform.opentargets.org/disease/{row['disease_id']}/associations", "data": {"lane": row["lane"], "disease_id": row["disease_id"], "disease_name": row["disease_name"], "score_interpretation": row["score_interpretation"], **target}})
    _same(bundle["sources"], sources, "Open Targets provenance")
    _same(bundle["association_count"], len(rows), "Open Targets lane count")
    _same(bundle["coverage"], "all_source_rows" if all(row["coverage"] == "all_source_rows" for row in rows) else "top_ranked_pages_only", "Open Targets coverage")
    return records, {"disease_lanes": summaries, "identity_namespace": "Ensembl_gene", "score_calculus_performed": False}


def _replay_gwas(bundle: dict[str, Any]) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    _exact(bundle, {"schema", "generated_at", "sources", "lanes", "source_set_digest", "lane_count", "association_count", "coverage", "provider", "credentials", "limitations", "bundle_digest"}, "GWAS bundle")
    rows = bundle["lanes"]
    if not isinstance(rows, list) or not 1 <= len(rows) <= 2:
        raise ArgumentError("GWAS disease lanes exceed their reviewed bounds")
    lanes = [row.get("lane") for row in rows if isinstance(row, dict)]
    if len(lanes) != len(rows) or lanes != [lane for lane in gwas.REVIEWED_GWAS_CATALOG_LANES if lane in lanes]:
        raise ArgumentError("GWAS disease selection is invalid or duplicated")
    records, sources, summaries = [], [], []
    for row in rows:
        _exact(row, {"source_id", "lane", "disease_id", "disease_name", "total_associations", "returned_associations", "omitted_associations", "page_size", "max_pages", "pages_requested", "coverage", "query_semantics", "associations"}, "GWAS lane")
        lane = row["lane"]
        total = _integer("GWAS source total", row["total_associations"], 0, 50_000_000)
        size = _integer("GWAS page size", row["page_size"], 1, 50)
        pages = _integer("GWAS page cap", row["max_pages"], 1, 5)
        returned = min(total, size * pages)
        total_pages = math.ceil(total / size) if total else 0
        expected = {"source_id": f"gwas_catalog_{lane}", "disease_id": gwas.REVIEWED_GWAS_CATALOG_LANES[lane][0], "disease_name": gwas.REVIEWED_GWAS_CATALOG_LANES[lane][1], "returned_associations": returned, "omitted_associations": total - returned, "pages_requested": min(total_pages, pages) if total_pages else 1, "coverage": "all_source_rows" if total_pages <= pages else "bounded_page_prefix", "query_semantics": "exact_ontology_trait_direct_matches_only"}
        for key, item in expected.items():
            _same(row[key], item, "GWAS " + key)
        associations = row["associations"]
        if not isinstance(associations, list) or len(associations) != returned:
            raise ArgumentError("GWAS returned associations differ from the bounded page prefix")
        seen = set()
        for association in associations:
            normalized = gwas._validate_projected_row(association, lane)
            _same(association, normalized, "GWAS association")
            if association["association_id"] in seen:
                raise ArgumentError("GWAS lane repeats an association identity")
            seen.add(association["association_id"])
            records.append({"kind": "gwas_association_annotation", "record_id": str(association["association_id"]), "source_id": row["source_id"], "uri": f"https://www.ebi.ac.uk/gwas/associations/{association['association_id']}", "data": {"lane": lane, "disease_id": row["disease_id"], **association}})
        query_digest = content_digest({"endpoint": gwas.REVIEWED_GWAS_CATALOG_ENDPOINT, "disease_id": row["disease_id"], "show_child_traits": False, "page_size": size, "max_pages": pages})
        sources.append(_source(gwas, row["source_id"], gwas.REVIEWED_GWAS_CATALOG_ENDPOINT, bundle["generated_at"], row, returned, query_digest=query_digest))
        summaries.append({key: row[key] for key in ("lane", "disease_id", "disease_name", "total_associations", "returned_associations", "omitted_associations", "pages_requested", "coverage", "query_semantics")})
    _same(bundle["sources"], sources, "GWAS provenance")
    _same(bundle["lane_count"], len(rows), "GWAS lane count")
    _same(bundle["association_count"], len(records), "GWAS association count")
    _same(bundle["coverage"], "all_source_rows" if all(row["coverage"] == "all_source_rows" for row in rows) else "bounded_page_prefix", "GWAS coverage")
    return records, {"disease_lanes": summaries, "mapped_gene_interpretation": "source_annotations_not_causal_assignments", "ancestry_and_population_applicability": "unassessed"}


def _replay(operation: str, bundle: Any) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    if not isinstance(bundle, dict) or bundle.get("schema") != _SCHEMAS[operation]:
        raise ArgumentError("molecular source bundle does not match its operation schema")
    module = _OWNERS[operation]
    if bundle.get("provider") != "none" or bundle.get("credentials") != ("none" if module is gene else "not_accepted") or bundle.get("limitations") != list(module._LIMITATIONS):
        raise ArgumentError("molecular source bundle boundary or limitations changed")
    module._timestamp(bundle.get("generated_at"), "molecular bundle clock")
    unsigned = {key: value for key, value in bundle.items() if key != "bundle_digest"}
    if bundle.get("bundle_digest") != content_digest(unsigned) or not isinstance(bundle.get("sources"), list) or bundle.get("source_set_digest") != content_digest(bundle["sources"]):
        raise ArgumentError("molecular source bundle or source-set digest is invalid")
    return {"cbioportal.studies": _replay_cbio, "ncbi_gene.summary": _replay_gene, "open_targets.associations": _replay_targets, "gwas_catalog.associations": _replay_gwas}[operation](bundle)


def analyze_glioma_molecular_coverage(*, question: str, requested_sources: Sequence[str], sources: Sequence[Mapping[str, Any]], max_citations: int = 128) -> dict[str, Any]:
    """Replay native reviewed bundles and derive separate bounded metadata inventories."""
    _integer("max_citations", max_citations, 1, 128)
    question = _question(question)
    if not isinstance(requested_sources, (list, tuple)) or not requested_sources or len(requested_sources) > 4 or any(not isinstance(item, str) or item not in _OWNERS for item in requested_sources) or len(set(requested_sources)) != len(requested_sources):
        raise ArgumentError("molecular requested sources must be unique reviewed operation IDs")
    inputs = _safe(sources, MAX_MOLECULAR_SOURCE_BYTES)
    if not isinstance(inputs, list) or len(inputs) > 4:
        raise ArgumentError("molecular sources must be a bounded list")
    partitions = {}
    for source in inputs:
        _exact(source, {"operation_id", "bundle"}, "molecular source input")
        operation = source["operation_id"]
        if not isinstance(operation, str) or operation not in requested_sources or operation in partitions:
            raise ArgumentError("molecular source is unrequested, duplicated, or unsupported")
        records, coverage = _replay(operation, source["bundle"])
        for row in records:
            row["record_digest"] = content_digest(row["data"])
            row["citation_id"] = "mol:" + content_digest({"operation_id": operation, "source_id": row["source_id"], "kind": row["kind"], "record_id": row["record_id"], "record_digest": row["record_digest"]})
        bundle = source["bundle"]
        partitions[operation] = {"operation_id": operation, "status": "records_present" if records else "retrieved_empty", "record_count": len(records), "bundle_digest": bundle["bundle_digest"], "source_set_digest": bundle["source_set_digest"], "generated_at": bundle["generated_at"], "sources": bundle["sources"], "coverage": coverage, "records": records, "source_limitations": bundle["limitations"]}
    missing = sorted(set(requested_sources) - partitions.keys())
    for operation in missing:
        partitions[operation] = {"operation_id": operation, "status": "not_supplied", "record_count": None, "bundle_digest": None, "coverage": None, "records": []}
    queues = [partitions[operation]["records"] for operation in sorted(partitions)]
    candidates = []
    for index in range(max((len(queue) for queue in queues), default=0)):
        for operation, queue in zip(sorted(partitions), queues):
            if index < len(queue):
                row = queue[index]
                candidates.append({"operation_id": operation, **{key: row[key] for key in ("citation_id", "kind", "record_id", "source_id", "record_digest", "uri")}})
    if len({row["citation_id"] for row in candidates}) != len(candidates):
        raise ArgumentError("molecular inventory repeats a citation identity")
    selected, omitted = candidates[:max_citations], candidates[max_citations:]
    unsigned = {"schema": GLIOMA_MOLECULAR_COVERAGE_SCHEMA, "scope": GLIOMA_MOLECULAR_SCOPE, "question": question, "question_digest": content_digest(question), "question_used_as_source_query": False,
        "requested_sources": sorted(requested_sources), "source_inputs": sorted(inputs, key=lambda row: row["operation_id"]), "source_partitions": [partitions[operation] for operation in sorted(partitions)], "missing_sources": missing,
        "status": "needs_evidence" if missing or not candidates else "metadata_ready_for_review", "max_citations": max_citations, "candidate_citation_count": len(candidates), "citation_index": selected,
        "omitted_citation_count": len(omitted), "omitted_citation_ids": [row["citation_id"] for row in omitted], "omitted_citation_ids_digest": content_digest([row["citation_id"] for row in omitted]),
        "gene_crosswalk": {"state": "not_supplied", "entrez_to_ensembl": [], "symbol_or_alias_matching_performed": False}, "cohort_overlap": {"state": "unassessed", "counts_pooled_across_sources": False},
        "negative_biological_evidence": {"state": "unassessed", "observations": []}, "measurement_values_retrieved": False, "scientific_support_established": False, "clinical_actions_authorized": False, "human_review_required": True, "limitations": list(_LIMITATIONS)}
    return _safe({**unsigned, "coverage_digest": content_digest(unsigned)})


def glioma_molecular_review_handoff(coverage: Mapping[str, Any]) -> dict[str, Any]:
    value = _safe(coverage)
    if not isinstance(value, dict) or value.get("schema") != GLIOMA_MOLECULAR_COVERAGE_SCHEMA:
        raise ArgumentError("molecular review requires the exact typed coverage artifact")
    unsigned = {key: item for key, item in value.items() if key != "coverage_digest"}
    if value.get("coverage_digest") != content_digest(unsigned):
        raise ArgumentError("molecular coverage digest does not match its contents")
    replayed = analyze_glioma_molecular_coverage(question=value.get("question"), requested_sources=value.get("requested_sources"), sources=value.get("source_inputs"), max_citations=value.get("max_citations"))
    _same(value, replayed, "molecular coverage artifact")
    payload = {key: item for key, item in value.items() if key != "source_inputs"}
    result = {"schema": GLIOMA_MOLECULAR_REVIEW_SCHEMA, "scope": GLIOMA_MOLECULAR_SCOPE, "status": "needs_evidence" if value["status"] == "needs_evidence" else "awaiting_scientific_human_review", "coverage": payload,
        "review_actions": ["Inspect native record identifiers, exact source links, timestamps, and omitted pages/citations.", "Keep profile availability separate from measured molecular results.", "Obtain an independently validated crosswalk before any Entrez–Ensembl identity join.", "Review GWAS ancestry, cohort overlap, confounding, and source evidence before making scientific claims."],
        "scientific_support_established": False, "clinical_actions_authorized": False, "human_review_required": True}
    return _safe({**result, "handoff_digest": content_digest(result)})


def register_glioma_molecular_workflow_operations(catalog: Any) -> Any:
    """Register local analysis executors separately; the default source catalogue is untouched."""
    from .workflow_integrations import WorkflowOperation, WorkflowOperationResult
    def analyze(arguments: Mapping[str, Any], **_kwargs: Any) -> Any:
        value = analyze_glioma_molecular_coverage(**dict(arguments))
        return WorkflowOperationResult(value, {"operation_id": GLIOMA_MOLECULAR_COVERAGE_OPERATION, "coverage_digest": value["coverage_digest"], "effect": "local_read_only", "human_review_required": True})
    def review(arguments: Mapping[str, Any], **_kwargs: Any) -> Any:
        value = glioma_molecular_review_handoff(arguments["coverage"])
        return WorkflowOperationResult(value, {"operation_id": GLIOMA_MOLECULAR_REVIEW_OPERATION, "handoff_digest": value["handoff_digest"], "effect": "local_read_only", "human_review_required": True})
    catalog.register(WorkflowOperation(GLIOMA_MOLECULAR_COVERAGE_OPERATION, "Replay reviewed molecular catalogue/gene/association metadata, preserve source citations and unknowns, and inventory availability without measurements or cross-ID joins.",
        {"type": "object", "required": ["question", "requested_sources", "sources"], "additionalProperties": False, "properties": {"question": {"type": "string", "minLength": 1, "maxLength": 8000}, "requested_sources": {"type": "array", "minItems": 1, "maxItems": 4, "uniqueItems": True, "items": {"type": "string", "enum": list(_OWNERS)}}, "sources": {"type": "array", "maxItems": 4, "items": {"type": "object", "required": ["operation_id", "bundle"], "additionalProperties": False, "properties": {"operation_id": {"type": "string", "enum": list(_OWNERS)}, "bundle": {"type": "object"}}}}, "max_citations": {"type": "integer", "minimum": 1, "maximum": 128, "default": 128}}},
        {"type": "object", "required": ["schema", "scope", "coverage_digest", "source_partitions", "citation_index"], "properties": {"schema": {"type": "string"}, "scope": {"type": "string"}, "coverage_digest": {"type": "string"}, "source_partitions": {"type": "array"}, "citation_index": {"type": "array"}}}, "local.glioma_molecular", implementation="local"), analyze)
    catalog.register(WorkflowOperation(GLIOMA_MOLECULAR_REVIEW_OPERATION, "Replay molecular metadata coverage and produce a citation-preserving review handoff without scientific or clinical authority.",
        {"type": "object", "required": ["coverage"], "additionalProperties": False, "properties": {"coverage": {"type": "object"}}}, {"type": "object", "required": ["schema", "status", "handoff_digest", "coverage"], "properties": {"schema": {"type": "string"}, "status": {"type": "string"}, "handoff_digest": {"type": "string"}, "coverage": {"type": "object"}}}, "local.glioma_molecular", implementation="local"), review)
    return catalog


def glioma_molecular_workflow_proposal(question: str, *, include_gwas: bool = True, association_page_size: int = 10, gwas_page_size: int = 10, gwas_max_pages: int = 1, max_citations: int = 128) -> dict[str, Any]:
    """Build the real-adapter DAG without source or provider dispatch."""
    if type(include_gwas) is not bool:
        raise ArgumentError("include_gwas must be a literal boolean")
    _integer("association_page_size", association_page_size, 1, 50)
    _integer("gwas_page_size", gwas_page_size, 1, 50)
    _integer("gwas_max_pages", gwas_max_pages, 1, 5)
    _integer("max_citations", max_citations, 1, 128)
    question = _question(question)
    steps = [
        {"id": "profiles", "operation": "cbioportal.studies", "arguments": {"study_ids": ["gbm_tcga", "lgg_tcga"]}, "depends_on": []},
        {"id": "genes", "operation": "ncbi_gene.summary", "arguments": {"gene_symbols": list(gene.REVIEWED_NCBI_GENE_CATALOGUE)}, "depends_on": []},
        {"id": "targets", "operation": "open_targets.associations", "arguments": {"lanes": ["gbm", "lgg"], "page_size": association_page_size}, "depends_on": []},
    ]
    if include_gwas:
        steps.append({"id": "gwas", "operation": "gwas_catalog.associations", "arguments": {"lanes": ["gbm", "glioma"], "page_size": gwas_page_size, "max_pages": gwas_max_pages}, "depends_on": []})
    source_steps = list(steps)
    steps.extend([
        {"id": "coverage", "operation": GLIOMA_MOLECULAR_COVERAGE_OPERATION, "arguments": {"question": question, "requested_sources": [step["operation"] for step in source_steps], "sources": [{"operation_id": step["operation"], "bundle": {"$ref": step["id"] + "#"}} for step in source_steps], "max_citations": max_citations}, "depends_on": [step["id"] for step in source_steps]},
        {"id": "review", "operation": GLIOMA_MOLECULAR_REVIEW_OPERATION, "arguments": {"coverage": {"$ref": "coverage#"}}, "depends_on": ["coverage"]},
    ])
    return {"name": "Glioma molecular catalogue and association review", "steps": steps, "outputs": ["review"]}
