"""Invariant tests for the repository-defined public glioma evidence composition."""

from copy import deepcopy
from pathlib import Path
import tempfile
import unittest
from urllib.parse import urlsplit

from prism_sdk.authoring import content_digest
from prism_sdk.errors import ArgumentError
from prism_sdk.glioma_workflow import (
    GLIOMA_RECONCILE_OPERATION,
    GLIOMA_REVIEW_OPERATION,
    GLIOMA_WORKFLOW_SCOPE,
    glioma_evidence_review_handoff,
    glioma_evidence_workflow_proposal,
    reconcile_glioma_evidence,
    register_glioma_workflow_operations,
)


def _bundle(operation):
    source_id = {"pubmed.search": "pubmed_glioma", "clinical_trials.search": "clinicaltrials_glioma", "gdc.projects": "gdc_gbm"}[operation]
    source = {"source_id": source_id, "authority": "offline transport fixture", "uri": "https://example.invalid/fixture", "retrieved_at": "2026-10-26T12:00:00Z"}
    if operation == "pubmed.search":
        return {"schema_version": "bioprism-neurosurgery-public-literature/0.1", "synthetic_data": False, "generated_at": source["retrieved_at"], "sources": [source], "records": [{"source_id": source_id, "pmid": "12345678", "title": "Fixture citation about glioma molecular evidence", "journal": "Fixture Journal", "publication_date": "2024-05", "doi": None, "abstract_text": None, "publication_types": [], "mesh_terms": [], "abstract_truncated": False}]}
    if operation == "clinical_trials.search":
        unsigned = {"schema": "bioprism-reviewed-clinical-trials-bundle/0.1", "generated_at": source["retrieved_at"], "sources": [source], "trials": [{"source_id": source_id, "nct_id": "NCT01234567", "title": "Registry metadata fixture", "overall_status": "RECRUITING", "phases": [], "last_update": "2026-09-01", "study_type": "INTERVENTIONAL", "enrollment_count": 0, "intervention_names": []}], "truncated": True, "reported_total_count": 4, "omitted_record_count": 3, "limitations": ["registry metadata does not establish benefit"]}
    else:
        unsigned = {"schema": "bioprism-reviewed-gdc-project-bundle/0.1", "generated_at": source["retrieved_at"], "sources": [source], "projects": [{"source_id": source_id, "project_id": "TCGA-GBM", "name": "TCGA project metadata fixture", "case_count": None, "file_count": 0, "released": False, "state": None, "data_categories": []}], "completeness": "unknown", "limitations": ["project summaries are not patient-level evidence"]}
    return {**unsigned, "bundle_digest": content_digest(unsigned)}


def _input(*operations):
    return [{"operation_id": operation, "bundle": _bundle(operation)} for operation in operations]


def _fixture_catalog(calls, *, failures=None, missing_source=None, reviewed_adapters=False):
    """Keep all eight real operation schemas while replacing every transport with a fixture."""
    from prism_sdk.workflow_integrations import (
        WorkflowOperationCatalog,
        WorkflowOperationResult,
        builtin_workflow_operation_catalog,
    )

    failures = {} if failures is None else failures
    definitions = builtin_workflow_operation_catalog().operations()
    catalog = WorkflowOperationCatalog()

    def executor_for(operation_id):
        def execute(arguments, *, approve_source_dispatch=False, retrieved_at=None):
            calls.append(operation_id)
            if approve_source_dispatch is not True:
                raise AssertionError("fixture source was entered without dispatch approval")
            if failures.get(operation_id, 0):
                failures[operation_id] -= 1
                raise RuntimeError("fixture source transport failed")
            if operation_id == missing_source:
                return WorkflowOperationResult({"sources": []}, {})
            if reviewed_adapters and operation_id in {"pubmed.search", "clinical_trials.search", "gdc.projects"}:
                return _reviewed_adapter_result(operation_id, arguments, retrieved_at)
            if operation_id in {"pubmed.search", "clinical_trials.search", "gdc.projects"}:
                bundle = _bundle(operation_id)
                if operation_id == "gdc.projects":
                    original = bundle["projects"][0]
                    source = bundle["sources"][0]
                    bundle["projects"] = []
                    bundle["sources"] = []
                    for project_id in arguments["project_ids"]:
                        source_id = "gdc_gbm" if project_id == "TCGA-GBM" else "gdc_lgg"
                        bundle["projects"].append({**original, "source_id": source_id, "project_id": project_id})
                        bundle["sources"].append({**source, "source_id": source_id})
                    bundle["bundle_digest"] = content_digest({key: value for key, value in bundle.items() if key != "bundle_digest"})
                return WorkflowOperationResult(bundle, {"transport": "fixture", "operation_id": operation_id})
            arrays = {
                "europe_pmc.search": ("publications",),
                "cbioportal.studies": ("studies", "molecular_profiles"),
                "ncbi_gene.summary": ("genes",),
                "open_targets.associations": ("associations",),
                "gwas_catalog.associations": ("lanes",),
            }[operation_id]
            return WorkflowOperationResult({"sources": [], **{name: [] for name in arrays}}, {"transport": "fixture"})
        return execute

    for definition in definitions:
        catalog.register(definition, executor_for(definition.operation_id))
    return register_glioma_workflow_operations(catalog)


def _reviewed_adapter_result(operation_id, arguments, retrieved_at):
    """Run the real source plan, parser, validation, and receipt code through local responses."""
    from prism_sdk.workflow_integrations import WorkflowOperationResult

    values = {key: tuple(value) if isinstance(value, list) else value for key, value in arguments.items()}
    values.update(transport_id=f"fixture.glioma.{operation_id}", transport_config_digest=content_digest({"fixture": operation_id}))
    if operation_id == "pubmed.search":
        from prism_sdk.reviewed_pubmed_retrieval import ReviewedPubMedRetrievalAdapter, ReviewedPubMedRetrievalConfig

        def fetch(url):
            if "esearch.fcgi" in url:
                return {"esearchresult": {"idlist": ["12345678"]}}
            if "esummary.fcgi" in url:
                return {"result": {"12345678": {"title": "Glioma citation transport fixture", "fulljournalname": "Fixture research journal", "pubdate": "2024 May"}}}
            return b"<PubmedArticleSet><PubmedArticle><MedlineCitation><PMID>12345678</PMID><Article><Abstract><AbstractText>Source text is review material.</AbstractText></Abstract></Article></MedlineCitation></PubmedArticle></PubmedArticleSet>"

        adapter = ReviewedPubMedRetrievalAdapter(ReviewedPubMedRetrievalConfig(**values), fetch=fetch)
    elif operation_id == "clinical_trials.search":
        from prism_sdk.reviewed_clinical_trials_retrieval import ReviewedClinicalTrialsRetrievalAdapter, ReviewedClinicalTrialsRetrievalConfig

        def fetch(url):
            return {"totalCount": 1, "studies": [{"protocolSection": {"identificationModule": {"nctId": "NCT01234567", "briefTitle": "Glioma registry transport fixture"}, "statusModule": {"overallStatus": "RECRUITING", "lastUpdatePostDateStruct": {"date": "2026-09"}}, "designModule": {"studyType": "INTERVENTIONAL", "enrollmentInfo": {"count": 0}}}}]}

        adapter = ReviewedClinicalTrialsRetrievalAdapter(ReviewedClinicalTrialsRetrievalConfig(**values), fetch=fetch)
    else:
        from prism_sdk.reviewed_gdc_project_retrieval import ReviewedGdcRetrievalAdapter, ReviewedGdcRetrievalConfig

        def fetch(url):
            project_id = urlsplit(url).path.rsplit("/", 1)[1]
            return {"data": {"project_id": project_id, "name": f"{project_id} project transport fixture", "disease_type": ["Gliomas"], "primary_site": ["Brain"], "state": "open", "released": False, "summary": {"case_count": None, "file_count": 0, "data_categories": []}}}

        adapter = ReviewedGdcRetrievalAdapter(ReviewedGdcRetrievalConfig(**values), fetch=fetch)
    result = adapter.execute(adapter.prepare(), approve_source_dispatch=True, retrieved_at=retrieved_at)
    receipt = result.receipt.to_dict() if hasattr(result.receipt, "to_dict") else result.receipt
    return WorkflowOperationResult(result.bundle, receipt)


class GliomaWorkflowTests(unittest.TestCase):
    def test_proposal_composes_exact_public_adapters_and_dependency_bound_local_handoff(self):
        proposal = glioma_evidence_workflow_proposal("Review glioma research coverage")
        steps = {row["id"]: row for row in proposal["steps"]}
        self.assertEqual(steps["literature"]["operation"], "pubmed.search")
        self.assertEqual(steps["trials"]["arguments"]["condition_lanes"], ["glioma"])
        self.assertEqual(steps["tcga"]["arguments"]["project_ids"], ["TCGA-GBM", "TCGA-LGG"])
        self.assertEqual(steps["reconcile"]["depends_on"], ["literature", "trials", "tcga"])
        self.assertEqual(steps["reconcile"]["arguments"]["sources"][0]["bundle"], {"$ref": "literature#"})
        self.assertEqual(steps["review"]["arguments"], {"reconciliation": {"$ref": "reconcile#"}})
        self.assertEqual(proposal["outputs"], ["review"])

    def test_optional_tcga_is_removed_from_both_dependencies_and_requested_evidence(self):
        proposal = glioma_evidence_workflow_proposal("Review public literature", include_tcga=False)
        self.assertNotIn("tcga", [row["id"] for row in proposal["steps"]])
        reconcile = proposal["steps"][-2]
        self.assertNotIn("gdc.projects", reconcile["arguments"]["requested_sources"])
        self.assertEqual(reconcile["depends_on"], ["literature", "trials"])

    def test_reconciliation_preserves_source_classes_citations_dates_and_missing_values(self):
        operations = ["pubmed.search", "clinical_trials.search", "gdc.projects"]
        inputs = _input(*operations)
        before = deepcopy(inputs)
        result = reconcile_glioma_evidence(question="Review glioma evidence gaps", requested_sources=operations, sources=inputs)
        self.assertEqual(inputs, before)
        self.assertEqual(result["status"], "metadata_ready_for_review")
        partitions = {row["operation_id"]: row for row in result["source_partitions"]}
        literature = partitions["pubmed.search"]["citations"][0]
        self.assertEqual(literature["citation_url"], "https://pubmed.ncbi.nlm.nih.gov/12345678/")
        self.assertEqual(literature["metadata"]["publication_date"], "2024-05")
        self.assertEqual(literature["missing_fields"], ["abstract_text", "doi"])
        self.assertIsNone(literature["metadata"]["doi"])
        trial = partitions["clinical_trials.search"]["citations"][0]
        self.assertEqual(trial["metadata"]["enrollment_count"], 0)
        self.assertEqual(trial["missing_fields"], [])
        self.assertTrue(partitions["clinical_trials.search"]["source_completeness"]["truncated"])
        project = partitions["gdc.projects"]["citations"][0]
        self.assertIsNone(project["metadata"]["case_count"])
        self.assertEqual(project["metadata"]["file_count"], 0)
        self.assertIs(project["metadata"]["released"], False)
        self.assertEqual(project["missing_fields"], ["case_count", "state"])
        self.assertEqual(len({row["evidence_class"] for row in result["citation_index"]}), 3)
        self.assertFalse(result["clinical_actions_authorized"])
        self.assertFalse(result["biological_evidence_promoted"])
        self.assertFalse(result["question_used_as_source_query"])

    def test_missing_source_has_unknown_record_count_and_cannot_be_ready_for_review(self):
        result = reconcile_glioma_evidence(question="Review glioma sources", requested_sources=["pubmed.search", "gdc.projects"], sources=_input("pubmed.search"))
        self.assertEqual(result["status"], "needs_evidence")
        self.assertEqual(result["missing_sources"], ["gdc.projects"])
        missing = result["source_partitions"][0]
        self.assertEqual(missing["operation_id"], "gdc.projects")
        self.assertIsNone(missing["record_count"])
        self.assertIsNone(missing["bundle_digest"])
        self.assertEqual(glioma_evidence_review_handoff(result)["status"], "needs_evidence")

    def test_present_empty_source_remains_distinct_from_missing_source(self):
        empty = _bundle("pubmed.search")
        empty["records"] = []
        result = reconcile_glioma_evidence(question="Review glioma sources", requested_sources=["pubmed.search"], sources=[{"operation_id": "pubmed.search", "bundle": empty}])
        self.assertEqual(result["missing_sources"], [])
        self.assertEqual(result["empty_sources"], ["pubmed.search"])
        self.assertEqual(result["source_partitions"][0]["record_count"], 0)
        self.assertEqual(result["status"], "needs_evidence")

    def test_bundle_tampering_unknown_sources_and_undeclared_record_sources_fail_closed(self):
        trials = _input("clinical_trials.search")
        trials[0]["bundle"]["trials"][0]["enrollment_count"] = 99
        with self.assertRaisesRegex(ArgumentError, "bundle digest"):
            reconcile_glioma_evidence(question="Review glioma", requested_sources=["clinical_trials.search"], sources=trials)
        with self.assertRaises(ArgumentError):
            reconcile_glioma_evidence(question="Review glioma", requested_sources=["unreviewed.source"], sources=[])
        records = _input("pubmed.search")
        records[0]["bundle"]["records"][0]["source_id"] = "invented"
        with self.assertRaisesRegex(ArgumentError, "no declared source"):
            reconcile_glioma_evidence(question="Review glioma", requested_sources=["pubmed.search"], sources=records)

    def test_duplicate_records_and_synthetic_inputs_do_not_inflate_evidence_counts(self):
        records = _input("pubmed.search")
        records[0]["bundle"]["records"] *= 2
        with self.assertRaisesRegex(ArgumentError, "repeats a source record"):
            reconcile_glioma_evidence(question="Review glioma", requested_sources=["pubmed.search"], sources=records)
        synthetic = _input("pubmed.search")
        synthetic[0]["bundle"]["synthetic_data"] = True
        with self.assertRaisesRegex(ArgumentError, "synthetic"):
            reconcile_glioma_evidence(question="Review glioma", requested_sources=["pubmed.search"], sources=synthetic)

    def test_patient_level_extensions_are_rejected_instead_of_forwarded_as_public_metadata(self):
        for location in ("record", "source"):
            inputs = _input("pubmed.search")
            bundle = inputs[0]["bundle"]
            target = bundle["records"][0] if location == "record" else bundle["sources"][0]
            target["patient_id"] = "private-fixture"
            with self.assertRaisesRegex(ArgumentError, "outside the public metadata contract"):
                reconcile_glioma_evidence(question="Review glioma", requested_sources=["pubmed.search"], sources=inputs)

    def test_review_preserves_every_citation_and_rejects_tampering_and_restamped_summary_drift(self):
        result = reconcile_glioma_evidence(question="Review glioma sources", requested_sources=["pubmed.search"], sources=_input("pubmed.search"))
        handoff = glioma_evidence_review_handoff(result)
        self.assertEqual(handoff["status"], "awaiting_human_review")
        self.assertEqual(handoff["citation_index"], result["citation_index"])
        self.assertEqual(handoff["reconciliation_digest"], result["reconciliation_digest"])
        self.assertTrue(handoff["human_review_required"])
        tampered = deepcopy(result)
        tampered["question"] = "different question"
        with self.assertRaisesRegex(ArgumentError, "digest"):
            glioma_evidence_review_handoff(tampered)
        tampered = deepcopy(result)
        tampered["citation_index"] = []
        unsigned = {key: value for key, value in tampered.items() if key != "reconciliation_digest"}
        tampered["reconciliation_digest"] = content_digest(unsigned)
        with self.assertRaisesRegex(ArgumentError, "citation index"):
            glioma_evidence_review_handoff(tampered)

    def test_local_operations_register_and_execute_without_source_approval(self):
        from prism_sdk.workflow_integrations import WorkflowOperationCatalog

        catalog = register_glioma_workflow_operations(WorkflowOperationCatalog())
        arguments = {"question": "Review glioma sources", "scope": GLIOMA_WORKFLOW_SCOPE, "requested_sources": ["pubmed.search"], "sources": _input("pubmed.search")}
        reconciliation = catalog.execute(GLIOMA_RECONCILE_OPERATION, arguments)
        handoff = catalog.execute(GLIOMA_REVIEW_OPERATION, {"reconciliation": reconciliation.value})
        self.assertEqual(handoff.value["status"], "awaiting_human_review")
        self.assertEqual(handoff.receipt["effect"], "local_read_only")
        self.assertEqual(len(catalog.operations()), 2)

    def test_scope_and_resource_bounds_reject_unsupported_execution(self):
        with self.assertRaisesRegex(ArgumentError, "scope"):
            reconcile_glioma_evidence(question="Review glioma", requested_sources=["pubmed.search"], sources=[], scope="clinical_treatment")
        for kwargs in ({"literature_limit": True}, {"trial_max_pages": 6}, {"include_tcga": "yes"}):
            with self.assertRaises(ArgumentError):
                glioma_evidence_workflow_proposal("Review glioma", **kwargs)
        with self.assertRaises(ArgumentError):
            glioma_evidence_workflow_proposal("x" * 8001)
        with self.assertRaises(ArgumentError):
            glioma_evidence_workflow_proposal("\ud800")

    def test_full_template_compiles_and_resolves_public_bundles_into_citation_preserving_review(self):
        from prism_sdk.workflow_builder import compile_workflow_plan, execute_workflow_plan

        calls = []
        catalog = _fixture_catalog(calls)
        proposal = glioma_evidence_workflow_proposal("Review glioma research coverage")
        plan = compile_workflow_plan(proposal, catalog, task="Review glioma research coverage")
        self.assertEqual(calls, [])
        unapproved = execute_workflow_plan(plan, catalog)
        self.assertEqual(unapproved.status, "approval_required")
        self.assertEqual(calls, [])
        run = execute_workflow_plan(plan, catalog, approved=True)
        self.assertEqual(run.status, "completed")
        self.assertEqual(calls, ["pubmed.search", "clinical_trials.search", "gdc.projects"])
        self.assertEqual(set(run.outputs), {"review"})
        handoff = run.outputs["review"]
        self.assertEqual(handoff["status"], "awaiting_human_review")
        self.assertEqual({row["record_id"] for row in handoff["citation_index"]}, {"12345678", "NCT01234567", "TCGA-GBM", "TCGA-LGG"})
        self.assertEqual(len(run.checkpoint["results"]), 5)
        self.assertEqual(run.checkpoint["results"]["reconcile"]["value"]["source_partitions"][0]["operation_id"], "clinical_trials.search")

    def test_disk_resume_reuses_completed_source_values_instead_of_refetching_them(self):
        from prism_sdk.workflow_builder import compile_workflow_plan, execute_workflow_plan, load_workflow_checkpoint, save_workflow_checkpoint

        initial_calls = []
        initial_catalog = _fixture_catalog(initial_calls)
        plan = compile_workflow_plan(glioma_evidence_workflow_proposal("Review glioma research coverage"), initial_catalog, task="Review glioma research coverage")
        partial = execute_workflow_plan(plan, initial_catalog, approved=True, max_step_calls=2)
        self.assertEqual(partial.status, "paused")
        self.assertEqual(initial_calls, ["pubmed.search", "clinical_trials.search"])
        with tempfile.TemporaryDirectory() as directory:
            store = Path(directory) / "public-glioma-run.json"
            save_workflow_checkpoint(store, partial.checkpoint)
            resumed_calls = []
            resumed_catalog = _fixture_catalog(resumed_calls)
            resumed = execute_workflow_plan(plan, resumed_catalog, approved=True, checkpoint=load_workflow_checkpoint(store))
        self.assertEqual(resumed.status, "completed")
        self.assertEqual(resumed_calls, ["gdc.projects"])
        self.assertEqual(resumed.checkpoint["attempts"]["literature"], 1)
        self.assertEqual(resumed.checkpoint["attempts"]["trials"], 1)
        self.assertEqual(resumed.outputs["review"]["status"], "awaiting_human_review")

    def test_source_failure_stops_reconciliation_and_requires_retry_without_refetching_successes(self):
        from prism_sdk.workflow_builder import compile_workflow_plan, execute_workflow_plan

        calls = []
        catalog = _fixture_catalog(calls, failures={"clinical_trials.search": 1})
        plan = compile_workflow_plan(glioma_evidence_workflow_proposal("Review glioma research coverage"), catalog, task="Review glioma research coverage")
        failed = execute_workflow_plan(plan, catalog, approved=True)
        self.assertEqual(failed.status, "failed")
        self.assertEqual(failed.checkpoint["failure"], {"step_id": "trials", "error_type": "RuntimeError"})
        self.assertEqual(set(failed.checkpoint["results"]), {"literature"})
        self.assertEqual(failed.outputs, {})
        held = execute_workflow_plan(plan, catalog, approved=True, checkpoint=failed.checkpoint)
        self.assertEqual(held.status, "retry_required")
        self.assertEqual(calls, ["pubmed.search", "clinical_trials.search"])
        resumed = execute_workflow_plan(plan, catalog, approved=True, checkpoint=held.checkpoint, retry_failed=True)
        self.assertEqual(resumed.status, "completed")
        self.assertEqual(calls, ["pubmed.search", "clinical_trials.search", "clinical_trials.search", "gdc.projects"])
        self.assertEqual(resumed.checkpoint["attempts"]["trials"], 2)

    def test_source_with_missing_bundle_rows_cannot_reach_reconciliation_or_review(self):
        from prism_sdk.workflow_builder import compile_workflow_plan, execute_workflow_plan

        calls = []
        catalog = _fixture_catalog(calls, missing_source="gdc.projects")
        plan = compile_workflow_plan(glioma_evidence_workflow_proposal("Review glioma research coverage"), catalog, task="Review glioma research coverage")
        failed = execute_workflow_plan(plan, catalog, approved=True)
        self.assertEqual(failed.status, "failed")
        self.assertEqual(failed.checkpoint["failure"]["step_id"], "tcga")
        self.assertEqual(set(failed.checkpoint["results"]), {"literature", "trials"})
        self.assertEqual(failed.outputs, {})

    def test_missing_registered_source_is_rejected_during_plan_compilation_before_dispatch(self):
        from prism_sdk.workflow_builder import compile_workflow_plan
        from prism_sdk.workflow_integrations import WorkflowOperationCatalog

        calls = []
        source_catalog = _fixture_catalog(calls)
        incomplete_catalog = WorkflowOperationCatalog()
        for operation in source_catalog.operations():
            if operation.operation_id != "gdc.projects":
                incomplete_catalog.register(operation, lambda *_args, **_kwargs: self.fail("compilation dispatched an executor"))
        with self.assertRaisesRegex(ArgumentError, "not registered"):
            compile_workflow_plan(glioma_evidence_workflow_proposal("Review glioma research coverage"), incomplete_catalog, task="Review glioma research coverage")
        self.assertEqual(calls, [])

    def test_full_template_uses_real_reviewed_plan_parsers_and_receipts_with_offline_transports(self):
        from prism_sdk.workflow_builder import compile_workflow_plan, execute_workflow_plan

        calls = []
        catalog = _fixture_catalog(calls, reviewed_adapters=True)
        proposal = glioma_evidence_workflow_proposal("Review glioma research coverage", literature_limit=1, trial_page_size=1, trial_max_pages=1)
        plan = compile_workflow_plan(proposal, catalog, task="Review glioma research coverage")
        run = execute_workflow_plan(plan, catalog, approved=True, retrieved_at="2026-10-26T12:00:00Z")
        self.assertEqual(run.status, "completed", run.to_dict())
        self.assertEqual(calls, ["pubmed.search", "clinical_trials.search", "gdc.projects"])
        results = run.checkpoint["results"]
        self.assertEqual(results["literature"]["receipt"]["request_count"], 3)
        self.assertEqual(results["trials"]["receipt"]["request_count"], 1)
        self.assertEqual(results["tcga"]["receipt"]["request_count"], 2)
        self.assertEqual(len(run.outputs["review"]["citation_index"]), 4)
        tcga = next(row for row in run.outputs["review"]["source_partitions"] if row["operation_id"] == "gdc.projects")
        self.assertIsNone(tcga["citations"][0]["metadata"]["case_count"])
        self.assertEqual(tcga["citations"][0]["metadata"]["file_count"], 0)


if __name__ == "__main__":
    unittest.main()
