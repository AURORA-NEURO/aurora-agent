"""Grounded draft invariants exercised with real LLMRuntime and no external dispatch."""

from copy import deepcopy
import json
import unittest

from prism_sdk.authoring import content_digest
from prism_sdk.errors import ArgumentError
from prism_sdk.glioma_workflow import reconcile_glioma_evidence, register_glioma_workflow_operations
from prism_sdk.glioma_workflow_synthesis import (
    GLIOMA_AUDIT_OPERATION,
    GLIOMA_SYNTHESIZE_OPERATION,
    LocalGliomaWorkflowSynthesizer,
    audit_glioma_synthesis_draft,
    glioma_grounded_workflow_proposal,
    prepare_glioma_synthesis_context,
    register_glioma_synthesis_operations,
)
from prism_sdk.llm_runtime import LLMRuntime, ProviderConfig, ProviderError
from prism_sdk.workflow_integrations import WorkflowOperationCatalog, WorkflowOperationResult, builtin_workflow_operation_catalog


def _reconciliation(*, empty=False, return_sources=False):
    timestamp = "2026-10-26T12:00:00Z"
    source = {"source_id": "pubmed_glioma", "authority": "PubMed", "uri": "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esearch.fcgi", "retrieved_at": timestamp, "content_sha256": "a" * 64, "record_count": 1}
    literature = {"schema_version": "bioprism-neurosurgery-public-literature/0.1", "generated_at": timestamp, "synthetic_data": False, "sources": [source], "records": [] if empty else [{"source_id": "pubmed_glioma", "specialty": "glioma", "pmid": "12345678", "title": "IDH1 and the public glioma molecular landscape", "journal": "Research journal", "publication_date": "2024-05", "doi": None, "abstract_text": "This source abstract must not appear in the metadata-only model context.", "abstract_truncated": False, "publication_types": ["Journal Article"], "mesh_terms": ["Glioma"]}]}
    trials = {"schema": "bioprism-reviewed-clinical-trials-bundle/0.1", "generated_at": timestamp, "sources": [{**source, "source_id": "clinicaltrials_glioma", "authority": "ClinicalTrials.gov", "uri": "https://clinicaltrials.gov/api/v2/studies"}], "trials": [] if empty else [{"source_id": "clinicaltrials_glioma", "nct_id": "NCT01234567", "title": "Registry metadata research fixture", "overall_status": "RECRUITING", "phases": ["PHASE2"], "last_update": None, "study_type": "INTERVENTIONAL", "enrollment_count": 0, "intervention_names": []}], "truncated": True, "reported_total_count": 3, "omitted_record_count": 2}
    gdc = {"schema": "bioprism-reviewed-gdc-project-bundle/0.1", "generated_at": timestamp, "sources": [{**source, "source_id": "gdc_gbm", "authority": "NCI GDC", "uri": "https://api.gdc.cancer.gov/projects/TCGA-GBM"}], "projects": [] if empty else [{"source_id": "gdc_gbm", "lane": "gbm", "project_id": "TCGA-GBM", "name": "TCGA glioblastoma project", "disease_types": ["Gliomas"], "primary_sites": ["Brain"], "state": "open", "released": False, "case_count": None, "file_count": 0, "data_categories": [{"data_category": "Transcriptome Profiling", "case_count": None, "file_count": 0}], "metadata_completeness": "unknown"}], "completeness": "unknown"}
    inputs = [{"operation_id": "pubmed.search", "bundle": literature}, {"operation_id": "clinical_trials.search", "bundle": trials}, {"operation_id": "gdc.projects", "bundle": gdc}]
    result = reconcile_glioma_evidence(question="Review public glioma research evidence gaps", requested_sources=["pubmed.search", "clinical_trials.search", "gdc.projects"], sources=inputs)
    return (result, inputs) if return_sources else result


def _record(context, operation):
    return next(row for row in context["records"] if row["operation_id"] == operation)


def _claim(record, field="title", *, state="metadata_observation", kind="source_observation"):
    metadata = record["metadata"]
    return {"claim_id": f"claim-{field}", "kind": kind, "scope": "public_record_metadata", "text": "Unverified text accompanying an exact source metadata assertion.", "evidence_state": state, "explicitly_hypothetical": kind == "research_hypothesis", "citations": [record["citation_id"]], "assertions": [{"citation_id": record["citation_id"], "field": field, "value": metadata[field]}]}


def _draft(context):
    literature = _record(context, "pubmed.search")
    gdc = _record(context, "gdc.projects")
    return {"answer": "The supplied public records suggest research themes that require source review; this is an unverified draft.", "unknowns": ["Molecular measurements and treatment outcomes are not provided."], "claims": [_claim(literature), _claim(gdc, "case_count", state="unknown"), _claim(gdc, "released", state="negative_evidence")]}


def _runtime(draft_builder=_draft):
    calls = []
    runtime = LLMRuntime()

    def model(request):
        calls.append(request)
        context = json.loads(request.messages[-1]["content"])
        return json.dumps(draft_builder(context))

    runtime.register_in_memory_provider("fixture_glioma", model)
    return runtime, calls


def _restamp(value, key):
    value[key] = content_digest({name: item for name, item in value.items() if name != key})
    return value


class GliomaWorkflowSynthesisTests(unittest.TestCase):
    def test_context_preserves_exact_public_record_fields_and_does_not_invent_abstracts(self):
        reconciliation = _reconciliation()
        context = prepare_glioma_synthesis_context(reconciliation)
        self.assertEqual(len(context["records"]), 3)
        article = _record(context, "pubmed.search")
        self.assertEqual(article["metadata"]["title"], "IDH1 and the public glioma molecular landscape")
        self.assertEqual(article["metadata"]["publication_date"], "2024-05")
        self.assertIsNone(article["metadata"]["doi"])
        self.assertTrue(article["abstract_available"])
        self.assertNotIn("source abstract must", json.dumps(context))
        self.assertNotIn("abstract_text", article["metadata"])
        self.assertEqual(article["record_digest"], next(row for row in reconciliation["citation_index"] if row["operation_id"] == "pubmed.search")["record_digest"])
        self.assertEqual(context["negative_biological_evidence"], {"state": "unassessed", "observations": []})
        self.assertEqual(context["omitted_record_count"], 0)
        self.assertLessEqual(len(json.dumps(context, ensure_ascii=False, separators=(",", ":")).encode()), context["max_context_bytes"])

    def test_provider_approval_is_independent_and_registration_is_pure(self):
        runtime, calls = _runtime()
        catalog = register_glioma_synthesis_operations(WorkflowOperationCatalog(), runtime=runtime, model="fixture-model", provider="fixture_glioma")
        self.assertEqual(calls, [])
        for approval in (False, True):
            with self.assertRaisesRegex(ArgumentError, "independent explicit provider approval"):
                catalog.execute(GLIOMA_SYNTHESIZE_OPERATION, {"reconciliation": _reconciliation()}, approve_source_dispatch=approval)
        self.assertEqual(calls, [])
        with self.assertRaises(ArgumentError):
            register_glioma_synthesis_operations(WorkflowOperationCatalog(), runtime=runtime, model="fixture-model", provider="fixture_glioma", approve_provider_call=1)

    def test_real_runtime_draft_audit_verifies_only_exact_metadata_and_retains_unknowns(self):
        runtime, calls = _runtime()
        synthesizer = LocalGliomaWorkflowSynthesizer(runtime, "fixture-model", "fixture_glioma")
        synthesis = synthesizer.synthesize(_reconciliation(), approve_provider_call=True)
        self.assertEqual(len(calls), 1)
        self.assertEqual(calls[0].tools, ())
        self.assertEqual(calls[0].temperature, 0)
        audit = audit_glioma_synthesis_draft(synthesis)
        self.assertEqual(audit["status"], "held_for_scientific_human_review")
        self.assertEqual([row["metadata_support"] for row in audit["claim_audits"]], ["exact_metadata_assertions_only", "exact_missing_metadata_only", "exact_negative_metadata_only_not_biological_evidence"])
        self.assertTrue(all(row["semantic_entailment"] == "unassessed" for row in audit["claim_audits"]))
        self.assertFalse(audit["scientific_support_established"])
        self.assertFalse(audit["clinical_actions_authorized"])
        self.assertTrue(audit["human_review_required"])
        self.assertTrue(audit["required_context_unknowns"]["missing_fields"])
        self.assertEqual(audit["claims_digest"], synthesis["claims_digest"])
        self.assertEqual(audit["model_binding_digest"], synthesis["model_binding_digest"])

    def test_outside_context_citation_is_blocked_even_when_present_in_original_reconciliation(self):
        original = _reconciliation()
        all_context = prepare_glioma_synthesis_context(original)
        omitted = _record(all_context, "pubmed.search")

        def omitted_claim(context):
            return {"answer": "Unverified draft", "unknowns": [], "claims": [_claim(omitted)]}

        runtime, calls = _runtime(omitted_claim)
        synthesis = LocalGliomaWorkflowSynthesizer(runtime, "fixture-model", "fixture_glioma", max_records=1).synthesize(original, approve_provider_call=True)
        self.assertEqual(len(calls), 1)
        self.assertEqual(synthesis["context"]["omitted_record_count"], 2)
        self.assertNotIn(omitted["citation_id"], [row["citation_id"] for row in synthesis["context"]["records"]])
        audit = audit_glioma_synthesis_draft(synthesis)
        self.assertEqual(audit["status"], "blocked")
        self.assertIsNone(audit["unverified_model_answer"])
        self.assertIn("citation_outside_exact_model_context", [row["code"] for row in audit["issues"]])

    def test_byte_budget_accounts_for_omitted_records_and_never_sends_partial_record_values(self):
        context = prepare_glioma_synthesis_context(_reconciliation(), max_context_bytes=4096)
        self.assertLessEqual(len(json.dumps(context, ensure_ascii=False, separators=(",", ":")).encode()), 4096)
        self.assertEqual(context["candidate_record_count"], len(context["records"]) + context["omitted_record_count"])
        self.assertGreater(context["omitted_record_count"], 0)
        self.assertEqual(len(context["omitted_citation_ids_digest"]), 64)
        for row in context["records"]:
            self.assertIn("record_digest", row)
            self.assertIn("metadata", row)

    def test_invented_but_well_formed_citation_and_modified_source_value_fail_the_audit(self):
        def changed(context):
            claim = _claim(_record(context, "pubmed.search"))
            claim["assertions"][0]["value"] = "A title never supplied to the model"
            other = deepcopy(claim)
            other["claim_id"] = "invented-id"
            other["citations"] = ["cite:" + "f" * 64]
            return {"answer": "Unverified draft", "unknowns": [], "claims": [claim, other]}

        runtime, _ = _runtime(changed)
        audit = audit_glioma_synthesis_draft(LocalGliomaWorkflowSynthesizer(runtime, "fixture", "fixture_glioma").synthesize(_reconciliation(), approve_provider_call=True))
        codes = {row["code"] for row in audit["issues"]}
        self.assertIn("citation_outside_exact_model_context", codes)
        self.assertIn("assertion_does_not_equal_supplied_metadata", codes)
        self.assertEqual(audit["status"], "blocked")

    def test_zero_and_missing_values_cannot_be_promoted_to_negative_biological_evidence(self):
        for field in ("file_count", "case_count"):
            def negative(context, field=field):
                return {"answer": "Unverified draft", "unknowns": [], "claims": [_claim(_record(context, "gdc.projects"), field, state="negative_evidence")]}

            runtime, _ = _runtime(negative)
            result = audit_glioma_synthesis_draft(LocalGliomaWorkflowSynthesizer(runtime, "fixture", "fixture_glioma").synthesize(_reconciliation(), approve_provider_call=True))
            self.assertEqual(result["status"], "blocked")
            self.assertIn("negative_metadata_requires_exact_false_not_missing_or_zero", [row["code"] for row in result["issues"]])
            self.assertEqual(result["negative_biological_evidence"]["state"], "unassessed")

    def test_clinical_action_patient_scope_and_scientific_authority_upgrade_block_final_draft(self):
        for mutation in ("clinical_action", "patient_case", "population_summary"):
            def outside(context, mutation=mutation):
                claim = _claim(_record(context, "pubmed.search"))
                if mutation == "patient_case":
                    claim["scope"] = mutation
                else:
                    claim["kind"] = mutation
                return {"answer": "This output must be held.", "unknowns": [], "claims": [claim]}

            runtime, _ = _runtime(outside)
            audit = audit_glioma_synthesis_draft(LocalGliomaWorkflowSynthesizer(runtime, "fixture", "fixture_glioma").synthesize(_reconciliation(), approve_provider_call=True))
            self.assertEqual(audit["status"], "blocked")
            self.assertIsNone(audit["unverified_model_answer"])

    def test_explicit_hypothesis_is_retained_as_unverified_instead_of_supported_science(self):
        def hypothesis(context):
            claim = _claim(_record(context, "pubmed.search"), state="research_hypothesis", kind="research_hypothesis")
            return {"answer": "Research hypothesis draft requiring independent review.", "unknowns": [], "claims": [claim]}

        runtime, _ = _runtime(hypothesis)
        audit = audit_glioma_synthesis_draft(LocalGliomaWorkflowSynthesizer(runtime, "fixture", "fixture_glioma").synthesize(_reconciliation(), approve_provider_call=True))
        self.assertEqual(audit["status"], "held_for_scientific_human_review")
        self.assertEqual(audit["claim_audits"][0]["metadata_support"], "citation_closed_scientific_text_unverified")
        self.assertFalse(audit["scientific_support_established"])

    def test_record_context_and_model_identity_tampering_is_rejected_before_claim_audit(self):
        runtime, _ = _runtime()
        synthesis = LocalGliomaWorkflowSynthesizer(runtime, "fixture-model", "fixture_glioma").synthesize(_reconciliation(), approve_provider_call=True)
        for mutation in ("record", "context", "model"):
            altered = deepcopy(synthesis)
            if mutation == "record":
                altered["context"]["records"][0]["metadata"]["title"] = "Changed metadata"
                _restamp(altered["context"], "context_digest")
                altered["context_digest"] = altered["context"]["context_digest"]
                altered["request_binding_digest"] = content_digest({"model_binding": altered["model_binding"], "context_digest": altered["context_digest"]})
            elif mutation == "context":
                altered["context"]["omitted_record_count"] = 99
            else:
                altered["model_binding"]["model"] = "different-model"
                altered["model_binding_digest"] = content_digest(altered["model_binding"])
            _restamp(altered, "synthesis_digest")
            with self.assertRaises(ArgumentError):
                audit_glioma_synthesis_draft(altered)
        altered = deepcopy(synthesis)
        altered["model_binding"]["model"] = "different-model"
        altered["model_binding_digest"] = content_digest(altered["model_binding"])
        altered["request_binding_digest"] = content_digest({"model_binding": altered["model_binding"], "context_digest": altered["context_digest"]})
        _restamp(altered, "synthesis_digest")
        with self.assertRaisesRegex(ArgumentError, "application model binding"):
            audit_glioma_synthesis_draft(altered, expected_model_binding=synthesis["model_binding"])

    def test_public_metadata_allowlist_rejects_private_fields_before_model_invocation(self):
        original = _reconciliation()
        for citation in original["citation_index"]:
            if citation["operation_id"] == "pubmed.search":
                citation["metadata"]["patient_id"] = "private-fixture"
        for partition in original["source_partitions"]:
            for citation in partition["citations"]:
                if citation["operation_id"] == "pubmed.search":
                    citation["metadata"]["patient_id"] = "private-fixture"
        _restamp(original, "reconciliation_digest")
        runtime, calls = _runtime()
        with self.assertRaisesRegex(ArgumentError, "public metadata contract"):
            LocalGliomaWorkflowSynthesizer(runtime, "fixture", "fixture_glioma").synthesize(original, approve_provider_call=True)
        self.assertEqual(calls, [])

    def test_empty_evidence_produces_a_no_call_hold_instead_of_a_synthetic_answer(self):
        runtime, calls = _runtime()
        synthesis = LocalGliomaWorkflowSynthesizer(runtime, "fixture", "fixture_glioma").synthesize(_reconciliation(empty=True), approve_provider_call=True)
        self.assertFalse(synthesis["provider_invoked"])
        self.assertIsNone(synthesis["draft"])
        self.assertEqual(calls, [])
        audit = audit_glioma_synthesis_draft(synthesis)
        self.assertEqual(audit["status"], "needs_evidence")
        self.assertIsNone(audit["claims_digest"])
        self.assertEqual(audit["required_context_unknowns"]["empty_sources"], ["clinical_trials.search", "gdc.projects", "pubmed.search"])

    def test_invalid_local_provider_scope_and_live_configuration_drift_are_rejected(self):
        runtime = LLMRuntime()
        runtime.register_provider(ProviderConfig(provider="remote_fixture", base_url="https://example.invalid/v1", requires_credential=False))
        with self.assertRaisesRegex(ArgumentError, "loopback or in-memory"):
            LocalGliomaWorkflowSynthesizer(runtime, "fixture", "remote_fixture")
        runtime, calls = _runtime()
        synthesis = LocalGliomaWorkflowSynthesizer(runtime, "fixture", "fixture_glioma")
        runtime.register_in_memory_provider("fixture_glioma", lambda _request: "{}", structured_output_mode="json_object")
        with self.assertRaisesRegex(ArgumentError, "configuration changed"):
            synthesis.synthesize(_reconciliation(), approve_provider_call=True)
        self.assertEqual(calls, [])

    def test_model_parse_and_schema_failures_propagate_without_fallback(self):
        runtime = LLMRuntime()
        runtime.register_in_memory_provider("fixture_glioma", lambda _request: '{"answer":"x","answer":"y","unknowns":[],"claims":[]}')
        with self.assertRaises((ArgumentError, ProviderError)):
            LocalGliomaWorkflowSynthesizer(runtime, "fixture", "fixture_glioma").synthesize(_reconciliation(), approve_provider_call=True)

    def test_type_confusion_cannot_use_boolean_false_as_the_observed_numeric_zero(self):
        def confuse(context):
            claim = _claim(_record(context, "gdc.projects"), "file_count")
            claim["assertions"][0]["value"] = False
            return {"answer": "Unverified draft", "unknowns": [], "claims": [claim]}

        runtime, _ = _runtime(confuse)
        audit = audit_glioma_synthesis_draft(LocalGliomaWorkflowSynthesizer(runtime, "fixture", "fixture_glioma").synthesize(_reconciliation(), approve_provider_call=True))
        self.assertEqual(audit["status"], "blocked")
        self.assertIn("assertion_does_not_equal_supplied_metadata", [row["code"] for row in audit["issues"]])

    def test_complete_source_template_synthesizes_only_after_retrieval_and_reconciliation(self):
        from prism_sdk.workflow_builder import compile_workflow_plan, execute_workflow_plan

        _, inputs = _reconciliation(return_sources=True)
        source_values = {row["operation_id"]: row["bundle"] for row in inputs}
        source_calls = []
        catalog = WorkflowOperationCatalog()

        def source_executor(operation_id):
            def execute(arguments, *, approve_source_dispatch=False, retrieved_at=None):
                self.assertTrue(approve_source_dispatch)
                source_calls.append(operation_id)
                return WorkflowOperationResult(source_values[operation_id], {"fixture": True})
            return execute

        for operation in builtin_workflow_operation_catalog().operations():
            if operation.operation_id in source_values:
                catalog.register(operation, source_executor(operation.operation_id))
        register_glioma_workflow_operations(catalog)
        runtime, model_calls = _runtime()
        register_glioma_synthesis_operations(catalog, runtime=runtime, model="fixture-model", provider="fixture_glioma", approve_provider_call=True)
        plan = compile_workflow_plan(glioma_grounded_workflow_proposal("Review public glioma evidence gaps"), catalog, task="Review public glioma evidence gaps")
        run = execute_workflow_plan(plan, catalog, approved=True)
        self.assertEqual(run.status, "completed")
        self.assertEqual(source_calls, ["pubmed.search", "clinical_trials.search", "gdc.projects"])
        self.assertEqual(len(model_calls), 1)
        self.assertEqual(run.outputs["audit"]["status"], "held_for_scientific_human_review")
        self.assertEqual(len(run.outputs["audit"]["citation_index"]), 3)

    def test_composed_template_executes_one_local_model_call_and_resumes_without_repeating_it(self):
        from prism_sdk.workflow_builder import compile_workflow_plan, execute_workflow_plan

        original = _reconciliation()
        calls = []
        runtime, model_calls = _runtime()
        catalog = register_glioma_workflow_operations(WorkflowOperationCatalog())
        # These exact reconciled inputs avoid any external source call in this integration test.
        def reconcile_again(arguments, **_kwargs):
            calls.append("source_fixture")
            return WorkflowOperationResult(original, {"fixture": True})

        from prism_sdk.workflow_integrations import WorkflowOperation
        catalog.register(WorkflowOperation("fixture.reconciliation", "Return a public reconciliation fixture", {"type": "object", "additionalProperties": False}, {"type": "object"}, "fixture", implementation="local"), reconcile_again)
        register_glioma_synthesis_operations(catalog, runtime=runtime, model="fixture-model", provider="fixture_glioma", approve_provider_call=True)
        proposal = glioma_grounded_workflow_proposal("Review public glioma evidence gaps")
        self.assertEqual(proposal["steps"][-2]["operation"], GLIOMA_SYNTHESIZE_OPERATION)
        self.assertEqual(proposal["steps"][-1]["operation"], GLIOMA_AUDIT_OPERATION)
        proposal["steps"] = [{"id": "reconcile", "operation": "fixture.reconciliation", "arguments": {}, "depends_on": []}, *proposal["steps"][-2:]]
        plan = compile_workflow_plan(proposal, catalog, task="Review public glioma evidence gaps")
        paused = execute_workflow_plan(plan, catalog, approved=True, max_step_calls=2)
        self.assertEqual(paused.status, "paused")
        self.assertEqual(len(model_calls), 1)
        resumed = execute_workflow_plan(plan, catalog, approved=True, checkpoint=paused.checkpoint)
        self.assertEqual(resumed.status, "completed")
        self.assertEqual(len(model_calls), 1)
        self.assertEqual(calls, ["source_fixture"])
        self.assertEqual(resumed.outputs["audit"]["status"], "held_for_scientific_human_review")

    def test_approval_is_invocation_authority_while_model_identity_binds_the_catalog(self):
        runtime, calls = _runtime()
        first = register_glioma_synthesis_operations(WorkflowOperationCatalog(), runtime=runtime, model="first", provider="fixture_glioma", approve_provider_call=False)
        approved = register_glioma_synthesis_operations(WorkflowOperationCatalog(), runtime=runtime, model="first", provider="fixture_glioma", approve_provider_call=True)
        other_model = register_glioma_synthesis_operations(WorkflowOperationCatalog(), runtime=runtime, model="second", provider="fixture_glioma", approve_provider_call=True)
        self.assertEqual(first.catalog_digest, approved.catalog_digest)
        self.assertNotEqual(approved.catalog_digest, other_model.catalog_digest)
        self.assertEqual(calls, [])
        with self.assertRaisesRegex(ArgumentError, "independent explicit provider approval"):
            first.execute(GLIOMA_SYNTHESIZE_OPERATION, {"reconciliation": _reconciliation()}, approve_source_dispatch=True)
        self.assertEqual(calls, [])
        self.assertEqual(approved.execute(GLIOMA_SYNTHESIZE_OPERATION, {"reconciliation": _reconciliation()}).value["status"], "draft_requires_audit")


if __name__ == "__main__":
    unittest.main()
