"""Real reviewed source parsers composed through the workflow catalogue, with offline fetches."""
from contextlib import contextmanager, ExitStack
from copy import deepcopy
from dataclasses import replace
from importlib import import_module
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from urllib.parse import parse_qs, urlencode, urlsplit

from prism_sdk.authoring import content_digest
from prism_sdk.errors import ArgumentError
from prism_sdk.glioma_molecular_workflow import (
    GLIOMA_MOLECULAR_COVERAGE_OPERATION,
    GLIOMA_MOLECULAR_REVIEW_OPERATION,
    analyze_glioma_molecular_coverage,
    glioma_molecular_review_handoff,
    glioma_molecular_workflow_proposal,
    register_glioma_molecular_workflow_operations,
)
from prism_sdk.workflow_builder import compile_workflow_plan, execute_workflow_plan, load_workflow_checkpoint, save_workflow_checkpoint
from prism_sdk.workflow_integrations import WorkflowOperationCatalog, builtin_workflow_operation_catalog

_AT = "2026-10-26T12:00:00Z"
_SPECS = {
    "cbioportal.studies": ("reviewed_cbioportal_study_retrieval", "ReviewedCBioPortalRetrievalAdapter"),
    "ncbi_gene.summary": ("reviewed_ncbi_gene_retrieval", "ReviewedNcbiGeneRetrievalAdapter"),
    "open_targets.associations": ("reviewed_open_targets_retrieval", "ReviewedOpenTargetsRetrievalAdapter"),
    "gwas_catalog.associations": ("reviewed_gwas_catalog_retrieval", "ReviewedGwasCatalogRetrievalAdapter"),
}


def _fetch(operation, calls, fail, empty_associations=False):
    def fetch(url, *args):
        calls[operation].append(url)
        if operation == fail:
            raise OSError("offline fixture source failed")
        if operation == "cbioportal.studies":
            path = urlsplit(url).path
            study = path.split("/studies/", 1)[1].split("/", 1)[0]
            if path.endswith("molecular-profiles"):
                return [{"studyId": study, "molecularProfileId": study + "_mutations", "name": "Mutation profile", "molecularAlterationType": "MUTATION_EXTENDED", "genericAssayType": None, "datatype": "MAF", "description": None, "showProfileInAnalysisTab": True, "patientLevel": None},
                        {"studyId": study, "molecularProfileId": study + "_mrna", "name": "Expression profile", "molecularAlterationType": "MRNA_EXPRESSION", "genericAssayType": None, "datatype": "CONTINUOUS", "description": "Catalogue description", "showProfileInAnalysisTab": False, "patientLevel": False}]
            return {"studyId": study, "name": study + " source fixture", "description": "Public catalogue metadata", "cancerTypeId": "gbm" if study == "gbm_tcga" else "lgg", "publicStudy": True, "pmid": "12345678", "allSampleCount": None if study == "gbm_tcga" else 0, "referenceGenome": "hg19", "importDate": "2024-01-01", "clinicalData": [{"patient_id": "raw field excluded by the source parser"}]}
        if operation == "ncbi_gene.summary":
            from prism_sdk.reviewed_ncbi_gene_retrieval import REVIEWED_NCBI_GENE_CATALOGUE
            ids = parse_qs(urlsplit(url).query)["id"][0].split(",")
            by_id = {value: key for key, value in REVIEWED_NCBI_GENE_CATALOGUE.items()}
            return {"header": {"type": "esummary", "version": "0.3"}, "result": {"uids": ids, **{gene_id: {"uid": gene_id, "name": by_id[gene_id], "description": by_id[gene_id] + " identity fixture", "chromosome": None, "maplocation": None, "otheraliases": "ZETA, ALPHA", "organism": {"taxid": 9606}} for gene_id in ids}}}
        if operation == "open_targets.associations":
            variables = json.loads(args[0])["variables"]
            disease_id, size = variables["efoId"], variables["pageSize"]
            name = "glioblastoma" if disease_id == "MONDO_0018177" else "low grade glioma"
            total = 0 if empty_associations else 3
            rows = [{"target": {"id": f"ENSG{index + 1:011d}", "approvedSymbol": "IDH1" if index == 0 else "TP53"}, "score": 0.75 - index * 0.1} for index in range(min(total, size))]
            return {"data": {"disease": {"id": disease_id, "name": name, "associatedTargets": {"count": total, "rows": rows}}}}
        query = parse_qs(urlsplit(url).query)
        disease_id, size, number = query["efo_id"][0], int(query["size"][0]), int(query["page"][0])
        total = 0 if empty_associations else 3
        pages = (total + size - 1) // size
        def page_url(index):
            return "https://www.ebi.ac.uk/gwas/rest/api/v2/associations?" + urlencode((("efo_id", disease_id), ("show_child_traits", "false"), ("page", index), ("size", size)))
        links = {"self": {"href": url}}
        if pages:
            links.update({"first": {"href": page_url(0)}, "last": {"href": page_url(pages - 1)}})
        if number + 1 < pages:
            links["next"] = {"href": page_url(number + 1)}
        offset = 100 if disease_id == "MONDO_0018177" else 200
        rows = [{"association_id": offset + index, "accession_id": "GCST00001", "pubmed_id": "12345678", "first_author": None, "p_value": 0 if index == 0 else 0.05, "efo_traits": [{"efo_id": disease_id, "efo_trait": "glioblastoma" if disease_id == "MONDO_0018177" else "glioma"}], "reported_trait": ["Glioma"], "mapped_genes": ["IDH1"], "locations": ["7:55012345"]} for index in range(number * size, min(total, (number + 1) * size))]
        return {"_links": links, "page": {"size": size, "totalElements": total, "totalPages": pages, "number": number}, **({"_embedded": {"associations": rows}} if total else {})}
    return fetch


@contextmanager
def _fixture_adapters(fail=None, empty_associations=False):
    calls = {operation: [] for operation in _SPECS}
    with ExitStack() as stack:
        for operation, (module_name, class_name) in _SPECS.items():
            module = import_module("prism_sdk." + module_name)
            adapter = getattr(module, class_name)
            original = adapter.__init__
            fetcher = _fetch(operation, calls, fail, empty_associations)
            def fixture_init(self, config, *, fetch=None, _original=original, _fetcher=fetcher, _operation=operation):
                if fetch is not None:
                    raise AssertionError("test wrapper unexpectedly supplied its own transport")
                fixture_config = replace(config, transport_id="fixture.molecular." + _operation, transport_config_digest=content_digest({"fixture": _operation}))
                _original(self, fixture_config, fetch=_fetcher)
            stack.enter_context(patch.object(adapter, "__init__", fixture_init))
        yield calls


def _restamp(bundle):
    bundle["source_set_digest"] = content_digest(bundle["sources"])
    bundle["bundle_digest"] = content_digest({key: value for key, value in bundle.items() if key != "bundle_digest"})


class GliomaMolecularWorkflowTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        proposal = glioma_molecular_workflow_proposal("Review public glioma molecular coverage", association_page_size=1, gwas_page_size=1, gwas_max_pages=1)
        with _fixture_adapters() as calls:
            catalog = builtin_workflow_operation_catalog()
            cls.inputs = [{"operation_id": step["operation"], "bundle": catalog.execute(step["operation"], step["arguments"], approve_source_dispatch=True, retrieved_at=_AT).value} for step in proposal["steps"][:-2]]
            cls.initial_calls = deepcopy(calls)

    def _analyze(self, inputs=None, **kwargs):
        return analyze_glioma_molecular_coverage(question="Review public glioma molecular coverage", requested_sources=list(_SPECS), sources=deepcopy(self.inputs if inputs is None else inputs), **kwargs)

    def test_native_source_parsers_are_replayed_and_metadata_stays_in_separate_planes(self):
        coverage = self._analyze()
        self.assertEqual(coverage["status"], "metadata_ready_for_review")
        partitions = {row["operation_id"]: row for row in coverage["source_partitions"]}
        studies = partitions["cbioportal.studies"]["coverage"]["studies"]
        self.assertIsNone(studies[0]["source_reported_sample_count"])
        self.assertEqual(studies[1]["source_reported_sample_count"], 0)
        self.assertEqual(studies[0]["alteration_type_counts"], {"MRNA_EXPRESSION": 1, "MUTATION_EXTENDED": 1})
        self.assertEqual(studies[0]["missing_description_count"], 1)
        self.assertEqual(studies[0]["unknown_patient_level_flag_count"], 1)
        self.assertEqual(studies[0]["measurement_values"], "not_retrieved")
        gene_rows = partitions["ncbi_gene.summary"]["records"]
        self.assertEqual(len(gene_rows), 12)
        self.assertEqual(gene_rows[0]["data"]["gene_id"], "3417")
        self.assertIsNone(gene_rows[0]["data"]["chromosome"])
        ranked = partitions["open_targets.associations"]["coverage"]["disease_lanes"]
        self.assertEqual(ranked[0]["omitted_associations"], 2)
        self.assertEqual(ranked[0]["score_interpretation"], "ranking_only_not_confidence")
        gwas_rows = partitions["gwas_catalog.associations"]["records"]
        self.assertEqual(gwas_rows[0]["data"]["p_value"], 0)
        self.assertNotEqual(gwas_rows[0]["data"]["p_value_state"], "certainty")
        self.assertFalse(coverage["gene_crosswalk"]["symbol_or_alias_matching_performed"])
        self.assertEqual(coverage["gene_crosswalk"]["entrez_to_ensembl"], [])
        self.assertFalse(coverage["cohort_overlap"]["counts_pooled_across_sources"])
        self.assertFalse(coverage["measurement_values_retrieved"])
        self.assertNotIn("patient_id", json.dumps(coverage))
        self.assertEqual({key: len(value) for key, value in self.initial_calls.items()}, {"cbioportal.studies": 4, "ncbi_gene.summary": 1, "open_targets.associations": 2, "gwas_catalog.associations": 2})

    def test_exact_source_record_citations_and_both_omission_layers_are_preserved(self):
        coverage = self._analyze(max_citations=4)
        self.assertEqual(len(coverage["citation_index"]), 4)
        self.assertEqual(len({row["operation_id"] for row in coverage["citation_index"]}), 4)
        self.assertGreater(coverage["omitted_citation_count"], 0)
        self.assertEqual(coverage["candidate_citation_count"], len(coverage["citation_index"]) + coverage["omitted_citation_count"])
        self.assertEqual(coverage["omitted_citation_count"], len(coverage["omitted_citation_ids"]))
        self.assertEqual(coverage["omitted_citation_ids_digest"], content_digest(coverage["omitted_citation_ids"]))
        for citation in coverage["citation_index"]:
            self.assertTrue(citation["uri"].startswith("https://"))
            self.assertEqual(len(citation["record_digest"]), 64)
        handoff = glioma_molecular_review_handoff(coverage)
        self.assertEqual(handoff["status"], "awaiting_scientific_human_review")
        self.assertEqual(handoff["coverage"]["omitted_citation_ids"], coverage["omitted_citation_ids"])

    def test_missing_sources_are_unknown_not_successful_empty_or_negative_evidence(self):
        coverage = self._analyze(self.inputs[:-1])
        self.assertEqual(coverage["status"], "needs_evidence")
        missing = next(row for row in coverage["source_partitions"] if row["status"] == "not_supplied")
        self.assertIsNone(missing["record_count"])
        self.assertIsNone(missing["coverage"])
        self.assertEqual(coverage["negative_biological_evidence"], {"state": "unassessed", "observations": []})

    def test_empty_retrieved_association_lanes_keep_zero_separate_from_negative_biology(self):
        with _fixture_adapters(empty_associations=True):
            catalog = builtin_workflow_operation_catalog()
            inputs = [{"operation_id": operation, "bundle": catalog.execute(operation, {"lanes": lanes, "page_size": 1, **({"max_pages": 1} if operation == "gwas_catalog.associations" else {})}, approve_source_dispatch=True, retrieved_at=_AT).value} for operation, lanes in (("open_targets.associations", ["gbm", "lgg"]), ("gwas_catalog.associations", ["gbm", "glioma"]))]
        coverage = analyze_glioma_molecular_coverage(question="Inventory empty public association lanes", requested_sources=[row["operation_id"] for row in inputs], sources=inputs)
        self.assertEqual(coverage["status"], "needs_evidence")
        self.assertEqual(coverage["missing_sources"], [])
        self.assertEqual(coverage["candidate_citation_count"], 0)
        self.assertEqual(coverage["negative_biological_evidence"]["state"], "unassessed")
        for partition in coverage["source_partitions"]:
            self.assertEqual(partition["status"], "retrieved_empty")
            self.assertEqual(partition["record_count"], 0)
            for lane in partition["coverage"]["disease_lanes"]:
                self.assertEqual(lane["total_associations"], 0)
                self.assertEqual(lane["returned_associations"], 0)
                self.assertEqual(lane["omitted_associations"], 0)
                self.assertEqual(lane["coverage"], "all_source_rows")
        self.assertEqual(glioma_molecular_review_handoff(coverage)["status"], "needs_evidence")

    def test_bundle_source_and_record_tampering_fail_even_when_outer_hashes_are_restamped(self):
        for operation, mutation in (("ncbi_gene.summary", "symbol"), ("cbioportal.studies", "patient"), ("open_targets.associations", "score"), ("gwas_catalog.associations", "count")):
            inputs = deepcopy(self.inputs)
            bundle = next(row["bundle"] for row in inputs if row["operation_id"] == operation)
            if mutation == "symbol":
                bundle["genes"][0]["symbol"] = "UNREVIEWED"
            elif mutation == "patient":
                bundle["studies"][0]["patient_id"] = "private"
            elif mutation == "score":
                bundle["associations"][0]["targets"][0]["association_score"] = True
            else:
                bundle["lanes"][0]["omitted_associations"] = 0
            _restamp(bundle)
            with self.subTest(operation=operation), self.assertRaises(ArgumentError):
                self._analyze(inputs)
        corrupted = deepcopy(self.inputs)
        corrupted[0]["bundle"]["sources"][0]["content_sha256"] = "0" * 64
        _restamp(corrupted[0]["bundle"])
        with self.assertRaises(ArgumentError):
            self._analyze(corrupted)

    def test_review_replays_source_inputs_and_rejects_restamped_summary_or_boundary_changes(self):
        coverage = self._analyze()
        for mutation in ("count", "crosswalk", "authority"):
            value = deepcopy(coverage)
            if mutation == "count":
                value["source_partitions"][0]["record_count"] += 1
            elif mutation == "crosswalk":
                value["gene_crosswalk"]["state"] = "verified"
            else:
                value["scientific_support_established"] = True
            value["coverage_digest"] = content_digest({key: item for key, item in value.items() if key != "coverage_digest"})
            with self.assertRaises(ArgumentError):
                glioma_molecular_review_handoff(value)

    def test_executable_real_adapter_dag_requires_approval_and_resumes_without_duplicate_reads(self):
        with _fixture_adapters() as calls, tempfile.TemporaryDirectory() as directory:
            catalog = register_glioma_molecular_workflow_operations(builtin_workflow_operation_catalog())
            proposal = glioma_molecular_workflow_proposal("Review public glioma molecular coverage", association_page_size=1, gwas_page_size=1)
            plan = compile_workflow_plan(proposal, catalog, task="Review public glioma molecular coverage")
            self.assertEqual(execute_workflow_plan(plan, catalog).status, "approval_required")
            self.assertTrue(all(not urls for urls in calls.values()))
            partial = execute_workflow_plan(plan, catalog, approved=True, max_step_calls=2, retrieved_at=_AT)
            self.assertEqual(partial.status, "paused")
            self.assertEqual(len(calls["cbioportal.studies"]), 4)
            self.assertEqual(len(calls["ncbi_gene.summary"]), 1)
            path = Path(directory) / "molecular-run.json"
            save_workflow_checkpoint(path, partial.checkpoint)
            run = execute_workflow_plan(plan, catalog, approved=True, checkpoint=load_workflow_checkpoint(path), retrieved_at=_AT)
            self.assertEqual(run.status, "completed", run.to_dict())
            self.assertEqual(run.outputs["review"]["status"], "awaiting_scientific_human_review")
            self.assertEqual(len(calls["cbioportal.studies"]), 4)
            self.assertEqual(len(calls["ncbi_gene.summary"]), 1)
            self.assertEqual(len(calls["open_targets.associations"]), 2)
            self.assertEqual(len(calls["gwas_catalog.associations"]), 2)

    def test_real_adapter_failure_stops_before_local_coverage_and_is_never_fabricated(self):
        with _fixture_adapters(fail="gwas_catalog.associations"):
            catalog = register_glioma_molecular_workflow_operations(builtin_workflow_operation_catalog())
            plan = compile_workflow_plan(glioma_molecular_workflow_proposal("Review public glioma molecular coverage", association_page_size=1, gwas_page_size=1), catalog, task="Review public glioma molecular coverage")
            failed = execute_workflow_plan(plan, catalog, approved=True, retrieved_at=_AT)
            self.assertEqual(failed.status, "failed")
            self.assertEqual(failed.checkpoint["failure"]["step_id"], "gwas")
            self.assertNotIn("coverage", failed.checkpoint["results"])
            self.assertEqual(failed.outputs, {})

    def test_optional_gwas_and_local_registration_do_not_change_default_source_catalogue(self):
        baseline = builtin_workflow_operation_catalog()
        digest = baseline.catalog_digest
        catalog = register_glioma_molecular_workflow_operations(WorkflowOperationCatalog())
        self.assertEqual(len(catalog.operations()), 2)
        self.assertEqual(builtin_workflow_operation_catalog().catalog_digest, digest)
        proposal = glioma_molecular_workflow_proposal("Review molecular metadata", include_gwas=False)
        self.assertNotIn("gwas", [step["id"] for step in proposal["steps"]])
        self.assertEqual(proposal["steps"][-2]["operation"], GLIOMA_MOLECULAR_COVERAGE_OPERATION)
        self.assertEqual(proposal["steps"][-1]["operation"], GLIOMA_MOLECULAR_REVIEW_OPERATION)
        self.assertFalse(any("gwas_catalog.associations" == name for name in proposal["steps"][-2]["arguments"]["requested_sources"]))

    def test_malformed_callers_strict_booleans_and_budgets_fail_before_any_dispatch(self):
        for kwargs in ({"max_citations": False}, {"max_citations": 129}):
            with self.assertRaises(ArgumentError):
                self._analyze(**kwargs)
        for kwargs in ({"include_gwas": 1}, {"association_page_size": True}, {"gwas_max_pages": 6}, {"gwas_page_size": 0}):
            with self.assertRaises(ArgumentError):
                glioma_molecular_workflow_proposal("Review molecular metadata", **kwargs)
        for question in (None, "", "\ud800", "x" * 8001):
            with self.assertRaises(ArgumentError):
                glioma_molecular_workflow_proposal(question)


if __name__ == "__main__":
    unittest.main()
