from __future__ import annotations

import json
import unittest
from typing import Any, Mapping

from prism_sdk import (
    ArgumentError,
    LLMRuntime,
    LocalNeurosurgicalAgent,
    NEUROSURGERY_GROUNDED_REAL_EVIDENCE_ACQUISITION_TOOL,
    NEUROSURGERY_GROUNDED_REAL_COVERAGE_TOOL,
    NEUROSURGERY_GROUNDED_SPECIALTY_EVIDENCE_MAP_TOOL,
    NEUROSURGERY_GROUNDED_REAL_FRESHNESS_TOOL,
    NEUROSURGERY_GROUNDED_LITERATURE_FRESHNESS_TOOL,
    NEUROSURGERY_GROUNDED_REAL_EVIDENCE_GRAPH_TOOL,
    NEUROSURGERY_GROUNDED_LITERATURE_REVIEW_QUEUE_TOOL,
    NEUROSURGERY_GROUNDED_LITERATURE_INTEGRITY_TOOL,
    NEUROSURGERY_GROUNDED_LITERATURE_EVIDENCE_ACQUISITION_TOOL,
    NEUROSURGERY_GROUNDED_REAL_MOLECULAR_COVERAGE_TOOL,
    NEUROSURGERY_GROUNDED_REAL_COHORT_LANDSCAPE_TOOL,
    NEUROSURGERY_GROUNDED_REAL_RECONCILIATION_TOOL,
    NEUROSURGERY_GROUNDED_REAL_RESEARCH_BRIEF_TOOL,
    NEUROSURGERY_GROUNDED_REAL_REVIEW_QUEUE_TOOL,
    NEUROSURGERY_GROUNDED_REAL_TRIAL_LANDSCAPE_TOOL,
    ProtocolError,
    ProviderConfig,
)
from prism_sdk.domain_tools import builtin_autonomous_domain_tool_profiles
from prism_sdk.models import ToolResult


class FakeClient:
    def __init__(self) -> None:
        self.calls: list[tuple[str, Mapping[str, Any]]] = []

    def list_tools(self) -> list[dict[str, Any]]:
        return [
            {"name": "neurosurgery_plan", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_session", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_catalogue", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_intake_plan", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_intake_mission", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_intake_portfolio", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_evidence_audit", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_specialty_evidence_map", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_case_asset_manifest", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_case_fhir_import", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_case_dicom_import", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_case_dicom_evidence_workflow", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_case_asset_review_disposition", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_evidence_synthesis", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_evidence_graph", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_real_data_coverage", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_real_data_cohort_landscape", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_real_data_reconciliation", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_real_data_freshness", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_real_data_diff", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_real_data_refresh_audit", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_real_data_review_queue", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_real_data_review_disposition", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_real_data_evidence_packet", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_real_data_reasoning_context", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_real_data_draft_audit", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_public_literature_evidence_packet", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_public_literature_reasoning_context", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_public_literature_draft_audit", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_public_literature_matrix", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_public_literature_freshness", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_public_literature_refresh_audit", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_literature_link_audit", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_public_literature_integrity_audit", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_public_literature_review_queue", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_public_literature_workbench", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_public_literature_portfolio", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_research_brief", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_research_plan", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_evidence_acquisition", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_real_data_query", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_real_data_trial_landscape", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_real_data_molecular_coverage", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_public_literature_query", "inputSchema": {"type": "object"}},
            {"name": "neurosurgery_mission", "inputSchema": {"type": "object"}},
            {"name": "unrelated_tool", "inputSchema": {"type": "object"}},
        ]

    def call_tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> ToolResult:
        args = dict(arguments or {})
        self.calls.append((name, args))
        if name == "neurosurgery_plan":
            payload = {"status": "ready_for_human_review", "real_data": None}
        elif name == "neurosurgery_catalogue":
            payload = {
                "schema_version": "bioprism-neurosurgery/0.1",
                "specialties": [{"specialty": "glioma"}],
                "tools": [{"capability": "safety_gate", "effect": "read_only"}],
                "provider": "none",
                "network": False,
                "effects": ["read_only"],
            }
        elif name == "neurosurgery_intake_plan":
            payload = {
                "schema_version": "bioprism-neurosurgery-intake-plan/0.1",
                "plan_digest": "i" * 64,
                "question_digest": "q" * 64,
                "candidates": [{"specialty": "glioma", "score_bps": 1000, "matched_terms": ["glioma"]}],
                "selected_specialty": "glioma",
                "confidence_bps": 1000,
                "abstained": False,
                "reason": "selected",
                "route": ["safety_gate", "glioma_molecular_panel", "human_review_hold"],
                "evidence_sources": ["real_glioma_snapshot", "pubmed_snapshot"],
                "reviewer_roles": ["neuro-oncology", "neurosurgery"],
                "next_actions": ["Construct a CaseRequest with an explicit research_synthesis purpose."],
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": ["routing is lexical vocabulary matching"],
            }
        elif name == "neurosurgery_intake_mission":
            payload = {
                "schema_version": "bioprism-neurosurgery-intake-mission/0.1",
                "intake": {
                    "schema_version": "bioprism-neurosurgery-intake-plan/0.1",
                    "plan_digest": "i" * 64,
                    "question_digest": "q" * 64,
                    "candidates": [],
                    "selected_specialty": "glioma",
                    "confidence_bps": 1000,
                    "abstained": False,
                    "reason": "selected",
                    "route": [],
                    "evidence_sources": ["real_glioma_snapshot"],
                    "reviewer_roles": [],
                    "next_actions": [],
                    "human_review_required": True,
                    "provider": "none",
                    "network": False,
                    "effect": "read_only",
                    "limitations": [],
                },
                "status": "ready_for_human_review",
                "request_digest": "r" * 64,
                "mission": {"status": "ready_for_human_review", "provider": "none", "network": False},
                "required_evidence": [],
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": [],
            }
        elif name == "neurosurgery_intake_portfolio":
            payload = {
                "schema_version": "bioprism-neurosurgery-intake-portfolio/0.1",
                "intake": {"schema_version": "bioprism-neurosurgery-intake-plan/0.1", "question_digest": "q" * 64, "abstained": False},
                "status": "ready_for_human_review",
                "request_digest": "r" * 64,
                "mission": None,
                "portfolio": {"specialty_count": 6, "provider": "none", "network": False, "synthetic_data": False},
                "selected_specialties": ["glioma", "cranial_base", "craniosynostosis", "encephalocele", "spina_bifida", "chiari_malformation"],
                "required_evidence": [],
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": [],
            }
        elif name == "neurosurgery_evidence_audit":
            payload = {
                "schema_version": "bioprism-neurosurgery-evidence-audit/0.1",
                "request_digest": "a" * 64,
                "specialty": "encephalocele",
                "required_observation_kinds": ["imaging"],
                "items": [],
                "missing_required_kinds": ["imaging"],
                "coverage_complete": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "temporal_alignment": {
                    "schema_version": "bioprism-neurosurgery-temporal-alignment/0.1",
                    "status": "unavailable",
                    "coverage_complete": False,
                    "provider": "none",
                    "network": False,
                    "effect": "read_only",
                },
            }
        elif name == "neurosurgery_specialty_evidence_map":
            payload = {
                "schema_version": "bioprism-neurosurgery-specialty-evidence-map/0.1",
                "map_digest": "m" * 64,
                "request_digest": "r" * 64,
                "specialty": "glioma",
                "dimensions": [],
                "required_dimension_count": 0,
                "complete_dimension_count": 0,
                "partial_dimension_count": 0,
                "not_collected_dimension_count": 0,
                "uninterpretable_dimension_count": 0,
                "conflicting_dimension_count": 0,
                "observed_observation_count": 0,
                "evidence_record_count": 0,
                "verified_evidence_record_count": 0,
                "missing_provenance_count": 0,
                "timestamped_observation_count": 0,
                "reviewer_questions": [],
                "state": "not_collected",
                "provenance_bound": True,
                "synthetic_data": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": [],
            }
        elif name == "neurosurgery_case_asset_manifest":
            payload = {
                "schema_version": "bioprism-neurosurgery-case-asset-manifest/0.1",
                "request_digest": "r" * 64,
                "manifest_digest": "m" * 64,
                "report_digest": "d" * 64,
                "specialty": "glioma",
                "asset_count": 0,
                "observed_asset_count": 0,
                "non_observed_asset_count": 0,
                "provenance_complete_asset_count": 0,
                "coverage": [],
                "requested_kinds": ["imaging_series"],
                "missing_requested_kinds": ["imaging_series"],
                "assets": [],
                "review_items": [],
                "omitted_review_item_count": 0,
                "truncated": False,
                "deidentified": True,
                "raw_values_retained": False,
                "synthetic_data": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": [],
            }
        elif name == "neurosurgery_case_fhir_import":
            payload = {
                "schema_version": "bioprism-neurosurgery-case-fhir-import/0.1",
                "request_digest": "r" * 64,
                "bundle_digest": "b" * 64,
                "hints_digest": "h" * 64,
                "report_digest": "d" * 64,
                "specialty": "glioma",
                "resource_count": 1,
                "projected_asset_count": 1,
                "unclassified_resource_count": 0,
                "manifest_report": {},
                "review_items": [],
                "omitted_review_item_count": 0,
                "truncated": False,
                "deidentified": True,
                "raw_values_retained": False,
                "synthetic_data": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": [],
            }
        elif name == "neurosurgery_case_dicom_import":
            payload = {
                "schema_version": "bioprism-neurosurgery-case-dicom-import/0.1",
                "request_digest": "r" * 64,
                "datasets_digest": "b" * 64,
                "report_digest": "d" * 64,
                "specialty": "glioma",
                "dataset_count": 1,
                "projected_series_count": 1,
                "unclassified_dataset_count": 0,
                "series": [],
                "manifest_report": {},
                "review_items": [],
                "omitted_review_item_count": 0,
                "truncated": False,
                "deidentified": True,
                "raw_values_retained": False,
                "synthetic_data": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": [],
            }
        elif name == "neurosurgery_case_dicom_evidence_workflow":
            payload = {
                "schema_version": "bioprism-neurosurgery-case-dicom-evidence-workflow/0.1",
                "workflow_digest": "w" * 64,
                "request_digest": "r" * 64,
                "specialty": "glioma",
                "query": args.get("query", {}),
                "dicom_import": {"schema_version": "bioprism-neurosurgery-case-dicom-import/0.1"},
                "evidence_synthesis": {},
                "evidence_program": {},
                "evidence_acquisition": {},
                "evidence_acquisition_session": {},
                "status": "ready_for_human_review",
                "human_review_required": True,
                "provenance_bound": True,
                "synthetic_data": False,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": [],
            }
        elif name == "neurosurgery_case_asset_review_disposition":
            payload = {
                "schema_version": "bioprism-neurosurgery-case-asset-review-disposition/0.1",
                "report_digest": "d" * 64,
                "disposition_digest": "x" * 64,
                "candidate_item_count": 2,
                "returned_item_count": 2,
                "omitted_item_count": 0,
                "submitted_decision_count": len(args.get("decisions", [])),
                "accepted_decision_count": len(args.get("decisions", [])),
                "resolved_decision_count": len(args.get("decisions", [])),
                "unresolved_decision_count": 0,
                "undecided_returned_item_count": 0,
                "pending_item_count": 0,
                "decisions": args.get("decisions", []),
                "unresolved_sequences": [],
                "undecided_sequences": [],
                "provenance_bound": True,
                "synthetic_data": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": [],
            }
        elif name == "neurosurgery_evidence_synthesis":
            payload = {
                "schema_version": "bioprism-neurosurgery-evidence-synthesis/0.1",
                "synthesis_digest": "s" * 64,
                "request_digest": "r" * 64,
                "specialty": "glioma",
                "generated_at": "2026-08-30T00:00:00Z",
                "query": args.get("query", {}),
                "case_observations": [],
                "case_audit": {"schema_version": "bioprism-neurosurgery-evidence-audit/0.1"},
                "references": [{"plane": "public_literature", "record_id": "PMID-12345678"}],
                "lanes": [],
                "real_data_summary": None,
                "public_literature_summary": {"bundle_digest": "f" * 64},
                "literature_link_audit": None,
                "links": [],
                "review_items": [],
                "reviewer_roles": ["neurosurgery"],
                "provenance_bound": True,
                "synthetic_data": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": ["alignment only"],
            }
        elif name == "neurosurgery_evidence_graph":
            payload = {
                "schema_version": "bioprism-neurosurgery-evidence-graph/0.1",
                "bundle_digest": "b" * 64,
                "graph_digest": "g" * 64,
                "specialty": "glioma",
                "query": args["query"],
                "nodes": [],
                "edges": [],
                "total_node_count": 0,
                "total_edge_count": 0,
                "omitted_node_count": 0,
                "omitted_edge_count": 0,
                "truncated": False,
                "root_count": 0,
                "connected_component_count": 0,
                "isolated_node_count": 0,
                "source_count": 0,
                "bundle_relationship_count": 0,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": ["explicit_crosswalk_only"],
            }
        elif name == "neurosurgery_real_data_coverage":
            payload = {
                "schema_version": "bioprism-neurosurgery-real-data-coverage/0.1",
                "bundle_digest": "b" * 64,
                "coverage_digest": "c" * 64,
                "generated_at": "2026-08-30T00:00:00Z",
                "query": args["query"],
                "total_record_count": 88,
                "matched_record_count": 4,
                "source_count": 5,
                "sources": [{"source_id": "fixture", "kind": "literature_index", "authority": "fixture authority", "uri": "https://example.test/fixture", "retrieved_at": "2026-08-30T00:00:00Z", "declared_record_count": 4, "observed_record_count": 4, "selected_record_count": 4}],
                "record_kind_counts": [{"record_kind": "literature_article", "count": 4}],
                "time_axes": [{"axis": "literature_publication_date", "observed_count": 4, "missing_count": 0, "earliest": "2026-01-01", "latest": "2026-08-30", "year_buckets": [{"year": 2026, "count": 4}]}],
                "portal_profile_type_counts": [{"alteration_type": "MUTATION", "count": 1}],
                "linkage": {"portal_study_count": 0, "portal_study_with_pmid_count": 0, "portal_study_without_pmid_count": 0, "portal_molecular_profile_count": 0, "explicit_profile_relationship_count": 0, "literature_article_count": 4, "literature_linked_to_portal_count": 0, "literature_without_portal_count": 4, "explicit_publication_relationship_count": 0, "literature_abstract_count": 4, "literature_abstract_missing_count": 0, "literature_abstract_truncated_count": 0},
                "gaps": [{"code": "fixture_gap", "count": 1, "description": "fixture review gap"}],
                "provenance_bound": True,
                "synthetic_data": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": ["fixture metadata only"],
            }
        elif name == "neurosurgery_real_data_cohort_landscape":
            payload = {
                "schema_version": "bioprism-neurosurgery-real-data-cohort-landscape/0.1",
                "landscape_digest": "l" * 64,
                "bundle_digest": "b" * 64,
                "generated_at": "2026-08-30T00:00:00Z",
                "query": args["query"],
                "total_matching_projects": 2,
                "returned_project_count": 2,
                "omitted_project_count": 0,
                "truncated": False,
                "project_rows": [],
                "total_released_case_inventory": 1133,
                "data_type_coverage": [],
                "shared_data_type_count": 0,
                "shared_data_types": [],
                "projects_with_data_type_metadata": 2,
                "projects_without_data_type_metadata": 0,
                "source_ids": ["gdc_tcga_gbm", "gdc_tcga_lgg"],
                "review_reasons": [],
                "provenance_bound": True,
                "synthetic_data": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": ["aggregate metadata only"],
            }
        elif name == "neurosurgery_real_data_reconciliation":
            payload = {
                "schema_version": "bioprism-neurosurgery-real-data-reconciliation/0.1",
                "reconciliation_digest": "r" * 64,
                "bundle_digest": "b" * 64,
                "generated_at": "2026-08-30T00:00:00Z",
                "query": args["query"],
                "counts": {
                    "portal_study_count": 7,
                    "portal_study_with_pmid_count": 6,
                    "portal_study_without_pmid_count": 1,
                    "portal_pmid_missing_literature_count": 0,
                    "shared_portal_pmid_count": 0,
                    "literature_article_count": 20,
                    "literature_with_doi_count": 20,
                    "shared_literature_doi_count": 0,
                },
                "candidate_issue_count": 0,
                "returned_issue_count": 0,
                "omitted_issue_count": 0,
                "truncated": False,
                "issues": [],
                "requires_review": False,
                "provenance_bound": True,
                "synthetic_data": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": ["identifier metadata only"],
            }
        elif name == "neurosurgery_real_data_diff":
            payload = {
                "schema_version": "bioprism-neurosurgery-real-data-diff/0.1",
                "before_bundle_digest": "b" * 64,
                "after_bundle_digest": "a" * 64,
                "diff_digest": "d" * 64,
                "before_generated_at": "2026-08-30T00:00:00Z",
                "after_generated_at": "2026-08-31T00:00:00Z",
                "query": args["query"],
                "before_record_count": 88,
                "after_record_count": 88,
                "record_counts": {"added": 0, "removed": 0, "changed": 1},
                "source_counts": {"added": 0, "removed": 0, "changed": 1},
                "total_change_count": 2,
                "returned_change_count": 2,
                "omitted_record_change_count": 0,
                "omitted_source_change_count": 0,
                "truncated": False,
                "record_changes": [],
                "source_changes": [],
                "provenance_bound": True,
                "synthetic_data": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": [],
            }
        elif name == "neurosurgery_real_data_refresh_audit":
            payload = {
                "schema_version": "bioprism-neurosurgery-real-data-refresh-audit/0.1",
                "audit_digest": "r" * 64,
                "before_bundle_digest": "b" * 64,
                "after_bundle_digest": "a" * 64,
                "before_generated_at": "2026-08-30T00:00:00Z",
                "after_generated_at": "2026-08-31T00:00:00Z",
                "query": args.get("query", {}),
                "diff": {"schema_version": "bioprism-neurosurgery-real-data-diff/0.1"},
                "coverage": {"schema_version": "bioprism-neurosurgery-real-data-coverage/0.1"},
                "freshness": None,
                "review_queue": {"schema_version": "bioprism-neurosurgery-real-data-review-queue/0.1"},
                "research_brief": {"schema_version": "bioprism-neurosurgery-real-data-research-brief/0.1", "source": "real_glioma"},
                "structural_change_detected": False,
                "source_identity_stable": True,
                "record_identity_stable": True,
                "requires_refresh_review": True,
                "review_reasons": [{"code": "metadata_obligations", "count": 1, "detail": "verify metadata"}],
                "provenance_bound": True,
                "synthetic_data": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": ["candidate snapshot is never accepted"],
            }
        elif name == "neurosurgery_real_data_freshness":
            payload = {
                "schema_version": "bioprism-neurosurgery-real-data-freshness/0.1",
                "bundle_digest": "b" * 64,
                "generated_at": "2026-08-30T00:00:00Z",
                "query": args["query"],
                "status": "stale",
                "source_count": 5,
                "current_source_count": 0,
                "stale_source_count": 5,
                "future_dated_source_count": 0,
                "sources": [],
                "freshness_digest": "f" * 64,
                "provenance_bound": True,
                "synthetic_data": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": [],
            }
        elif name == "neurosurgery_real_data_review_queue":
            payload = {
                "schema_version": "bioprism-neurosurgery-real-data-review-queue/0.1",
                "bundle_digest": "b" * 64,
                "queue_digest": "q" * 64,
                "generated_at": "2026-08-30T00:00:00Z",
                "query": args["query"],
                "source_count": 5,
                "record_count": 88,
                "candidate_item_count": 15,
                "returned_item_count": 2,
                "omitted_item_count": 13,
                "truncated": True,
                "items": [],
                "provenance_bound": True,
                "synthetic_data": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": [],
            }
        elif name == "neurosurgery_real_data_review_disposition":
            payload = {
                "schema_version": "bioprism-neurosurgery-real-data-review-disposition/0.1",
                "bundle_digest": "b" * 64,
                "queue_digest": "q" * 64,
                "disposition_digest": "d" * 64,
                "candidate_item_count": 15,
                "queue_returned_item_count": 2,
                "queue_omitted_item_count": 13,
                "submitted_decision_count": len(args.get("decisions", [])),
                "accepted_decision_count": len(args.get("decisions", [])),
                "resolved_decision_count": len(args.get("decisions", [])),
                "unresolved_decision_count": 0,
                "undecided_returned_item_count": 1,
                "pending_item_count": 13,
                "decisions": args.get("decisions", []),
                "unresolved_task_ids": [],
                "undecided_task_ids": [],
                "provenance_bound": True,
                "synthetic_data": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": [],
            }
        elif name == "neurosurgery_real_data_evidence_packet":
            payload = {
                "schema_version": "bioprism-neurosurgery-real-data-evidence-packet/0.4",
                "packet_digest": "p" * 64,
                "bundle_digest": "b" * 64,
                "generated_at": "2026-08-30T00:00:00Z",
                "query": args.get("query", {}),
                "summary": {"bundle_digest": "b" * 64},
                "coverage": {"coverage_digest": "c" * 64},
                "graph": {"graph_digest": "g" * 64},
                "data_query": {"total_matches": 4, "query": args.get("query", {}).get("query", {})},
                "trial_landscape": {
                    "schema_version": "bioprism-neurosurgery-real-data-trial-landscape/0.1",
                    "landscape_digest": "l" * 64,
                    "bundle_digest": "b" * 64,
                    "generated_at": "2026-08-30T00:00:00Z",
                    "query": {"query": {}, "max_interventions": 128},
                    "total_matching_trials": 5,
                    "returned_trial_count": 5,
                    "omitted_trial_count": 0,
                    "truncated": False,
                    "status_counts": [],
                    "phase_counts": [],
                    "phase_annotated_trial_count": 5,
                    "study_type_counts": [],
                    "intervention_counts": [],
                    "distinct_intervention_count": 0,
                    "omitted_intervention_count": 0,
                    "intervention_truncated": False,
                    "missing_phase_count": 0,
                    "missing_last_update_count": 0,
                    "missing_study_type_count": 0,
                    "missing_enrollment_count": 0,
                    "missing_intervention_count": 0,
                    "earliest_last_update": None,
                    "latest_last_update": None,
                    "source_ids": [],
                    "review_reasons": [],
                    "provenance_bound": True,
                    "synthetic_data": False,
                    "human_review_required": True,
                    "provider": "none",
                    "network": False,
                    "effect": "read_only",
                    "limitations": ["metadata only"],
                },
                "molecular_coverage": {
                    "schema_version": "bioprism-neurosurgery-real-data-molecular-coverage/0.1",
                    "coverage_digest": "m" * 64,
                    "bundle_digest": "b" * 64,
                    "generated_at": "2026-08-30T00:00:00Z",
                    "query": {"query": {"limit": 128}, "max_studies": 128},
                    "total_matching_profile_count": 54,
                    "returned_profile_count": 54,
                    "omitted_profile_count": 0,
                    "truncated": False,
                    "distinct_returned_study_count": 7,
                    "emitted_study_count": 7,
                    "omitted_study_count": 0,
                    "study_rows_truncated": False,
                    "emitted_profile_count": 54,
                    "study_rows": [],
                    "alteration_type_counts": [],
                    "datatype_counts": [],
                    "patient_level_profile_count": 0,
                    "analysis_visible_profile_count": 0,
                    "description_present_count": 54,
                    "missing_description_count": 0,
                    "missing_alteration_type_count": 0,
                    "missing_datatype_count": 0,
                    "missing_study_link_count": 0,
                    "source_ids": [],
                    "review_reasons": [],
                    "provenance_bound": True,
                    "synthetic_data": False,
                    "human_review_required": True,
                    "provider": "none",
                    "network": False,
                    "effect": "read_only",
                    "limitations": ["metadata only"],
                },
                "review_queue": {"candidate_item_count": 15},
                "source_count": 5,
                "record_count": 88,
                "query_match_count": 4,
                "open_review_obligation_count": 15,
                "explicit_crosswalk_edge_count": 60,
                "provenance_bound": True,
                "synthetic_data": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": [],
            }
        elif name == "neurosurgery_real_data_draft_audit":
            payload = {
                "schema_version": "bioprism-neurosurgery-real-data-draft-audit/0.1",
                "draft_digest": "d" * 64,
                "packet_digest": "p" * 64,
                "bundle_digest": "b" * 64,
                "generated_at": "2026-08-30T00:00:00Z",
                "packet": {"packet_digest": "p" * 64},
                "claims": [],
                "claim_count": len(args.get("claims", [])),
                "grounded_claim_count": len(args.get("claims", [])),
                "blocked_claim_count": 0,
                "status": "grounded_for_human_review",
                "provenance_bound": True,
                "synthetic_data": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": [],
            }
        elif name == "neurosurgery_public_literature_evidence_packet":
            payload = {
                "schema_version": "bioprism-neurosurgery-public-literature-evidence-packet/0.1",
                "packet_digest": "p" * 64,
                "bundle_digest": "f" * 64,
                "generated_at": "2026-08-30T00:00:00Z",
                "query": args.get("query", {}),
                "summary": {"bundle_digest": "f" * 64},
                "query_result": {"total_matches": 1, "returned_matches": 1, "hits": [{"pmid": "1"}]},
                "source_count": 1,
                "record_count": 145,
                "query_match_count": 1,
                "abstract_count": 138,
                "abstract_truncated_count": 0,
                "specialty_counts": [],
                "provenance_bound": True,
                "synthetic_data": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": [],
            }
        elif name == "neurosurgery_public_literature_freshness":
            payload = {
                "schema_version": "bioprism-neurosurgery-real-data-freshness/0.1",
                "bundle_digest": "f" * 64,
                "generated_at": "2026-08-30T00:00:00Z",
                "query": args["query"],
                "status": "current",
                "source_count": 1,
                "current_source_count": 1,
                "stale_source_count": 0,
                "future_dated_source_count": 0,
                "sources": [],
                "freshness_digest": "f" * 64,
                "provenance_bound": True,
                "synthetic_data": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": [],
            }
        elif name == "neurosurgery_public_literature_refresh_audit":
            payload = {
                "schema_version": "bioprism-neurosurgery-public-literature-refresh-audit/0.1",
                "audit_digest": "r" * 64,
                "before_bundle_digest": "b" * 64,
                "after_bundle_digest": "a" * 64,
                "before_generated_at": "2026-08-30T00:00:00Z",
                "after_generated_at": "2026-08-31T00:00:00Z",
                "query": args.get("query", {}),
                "before_summary": {},
                "after_summary": {},
                "diff": {"schema_version": "bioprism-neurosurgery-public-literature-refresh-diff/0.1"},
                "matrix": {"schema_version": "bioprism-neurosurgery-public-literature-matrix/0.1"},
                "freshness": None,
                "structural_change_detected": False,
                "specialty_coverage_changed": False,
                "source_identity_stable": True,
                "record_identity_stable": True,
                "requires_refresh_review": False,
                "review_reasons": [],
                "provenance_bound": True,
                "synthetic_data": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": [],
            }
        elif name == "neurosurgery_literature_link_audit":
            payload = {
                "schema_version": "bioprism-neurosurgery-literature-link-audit/0.1",
                "audit_digest": "l" * 64,
                "real_data_bundle_digest": "r" * 64,
                "public_literature_bundle_digest": "p" * 64,
                "real_data_generated_at": "2026-08-30T00:00:00Z",
                "public_literature_generated_at": "2026-08-30T00:00:00Z",
                "query": args.get("query", {}),
                "real_data_summary": {},
                "public_literature_summary": {},
                "counts": {
                    "real_literature_records": 20,
                    "selected_public_literature_records": 25,
                    "linked_real_records": 12,
                    "linked_public_records": 12,
                    "unmatched_real_records": 8,
                    "unmatched_public_records": 13,
                    "pmid_match_count": 12,
                    "doi_match_count": 12,
                    "metadata_mismatch_count": 0,
                    "identifier_conflict_count": 0,
                },
                "links": [],
                "unmatched_real_pmids": [],
                "unmatched_public_pmids": [],
                "omitted_link_count": 0,
                "omitted_unmatched_real_count": 0,
                "omitted_unmatched_public_count": 0,
                "truncated": False,
                "requires_link_review": True,
                "review_reasons": [],
                "provenance_bound": True,
                "synthetic_data": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": [],
            }
        elif name == "neurosurgery_public_literature_integrity_audit":
            payload = {
                "schema_version": "bioprism-neurosurgery-public-literature-integrity-audit/0.1",
                "audit_digest": "i" * 64,
                "bundle_digest": "p" * 64,
                "generated_at": "2026-08-30T00:00:00Z",
                "query": args.get("query", {}),
                "summary": {},
                "counts": {
                    "selected_record_count": 145,
                    "selected_source_count": 6,
                    "unique_pmid_count": 145,
                    "doi_count": 145,
                    "missing_doi_count": 0,
                    "abstract_count": 138,
                    "missing_abstract_count": 7,
                    "abstract_truncated_count": 0,
                    "empty_publication_type_count": 0,
                    "empty_mesh_term_count": 84,
                    "duplicate_doi_group_count": 0,
                    "cross_specialty_duplicate_doi_group_count": 0,
                },
                "issues": [{"code": "missing_abstract", "specialty": "glioma", "pmid": "PMID-12345678", "source_id": "pubmed_glioma", "related_pmids": [], "detail": "abstract metadata is absent"}],
                "omitted_issue_count": 0,
                "truncated": False,
                "requires_integrity_review": True,
                "review_reasons": [],
                "provenance_bound": True,
                "synthetic_data": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": [],
            }
        elif name == "neurosurgery_public_literature_review_queue":
            payload = {
                "schema_version": "bioprism-neurosurgery-public-literature-review-queue/0.1",
                "bundle_digest": "p" * 64,
                "queue_digest": "q" * 64,
                "integrity_audit_digest": "i" * 64,
                "candidate_item_count": 3,
                "returned_item_count": 3,
                "omitted_item_count": 0,
                "omitted_integrity_issue_count": 0,
                "truncated": False,
                "items": [],
                "provenance_bound": True,
                "synthetic_data": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": [],
            }
        elif name == "neurosurgery_public_literature_workbench":
            payload = {
                "schema_version": "bioprism-neurosurgery-public-literature-workbench/0.1",
                "workbench_digest": "w" * 64,
                "bundle_digest": "p" * 64,
                "generated_at": "2026-08-30T00:00:00Z",
                "query": args.get("query", {}),
                "lanes": [{
                    "specialty": "glioma",
                    "profile": {
                        "specialty": "glioma",
                        "identity_axes": ["molecular"],
                        "spatial_axes": ["anatomic"],
                        "temporal_axes": ["longitudinal"],
                        "evidence_questions": ["what is observed?"],
                        "confounders": ["sampling"],
                        "human_review_roles": ["neuro-oncology"],
                    },
                    "source_ids": ["pubmed_glioma"],
                    "record_count": 25,
                    "abstract_count": 25,
                    "abstract_truncated_count": 0,
                    "missing_doi_count": 0,
                    "missing_abstract_count": 0,
                    "empty_publication_type_count": 0,
                    "empty_mesh_term_count": 3,
                    "review_issue_count": 3,
                    "omitted_review_issue_count": 0,
                    "truncated": False,
                    "integrity_audit_digest": "i" * 64,
                    "review_reasons": [],
                }],
                "specialty_count": 1,
                "non_empty_lane_count": 1,
                "empty_lane_specialties": [],
                "total_record_count": 25,
                "total_review_issue_count": 3,
                "omitted_review_issue_count": 0,
                "truncated_lane_count": 0,
                "freshness": None,
                "provenance_bound": True,
                "synthetic_data": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": [],
            }
        elif name == "neurosurgery_public_literature_portfolio":
            payload = {
                "schema_version": "bioprism-neurosurgery-public-literature-portfolio/0.1",
                "portfolio_digest": "o" * 64,
                "bundle_digest": "p" * 64,
                "generated_at": "2026-08-30T00:00:00Z",
                "query": args.get("query", {}),
                "lanes": [{
                    "specialty": "glioma",
                    "workbench": {"specialty": "glioma", "record_count": 25},
                    "query_result": {"total_matches": 25, "returned_matches": 2, "truncated": True},
                    "review_queue": {"candidate_item_count": 3, "returned_item_count": 2, "omitted_item_count": 1, "truncated": True},
                }],
                "specialty_count": 1,
                "non_empty_lane_count": 1,
                "empty_lane_specialties": [],
                "total_match_count": 25,
                "total_returned_count": 2,
                "total_review_issue_count": 3,
                "total_review_item_count": 3,
                "omitted_review_item_count": 1,
                "truncated_lane_count": 1,
                "freshness": None,
                "provenance_bound": True,
                "synthetic_data": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": [],
            }
        elif name == "neurosurgery_public_literature_draft_audit":
            payload = {
                "schema_version": "bioprism-neurosurgery-public-literature-draft-audit/0.1",
                "draft_digest": "d" * 64,
                "packet_digest": "p" * 64,
                "bundle_digest": "f" * 64,
                "generated_at": "2026-08-30T00:00:00Z",
                "packet": {"packet_digest": "p" * 64},
                "claims": [],
                "claim_count": len(args.get("claims", [])),
                "grounded_claim_count": len(args.get("claims", [])),
                "blocked_claim_count": 0,
                "status": "grounded_for_human_review",
                "provenance_bound": True,
                "synthetic_data": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": [],
            }
        elif name == "neurosurgery_public_literature_reasoning_context":
            payload = {
                "schema_version": "bioprism-neurosurgery-public-literature-reasoning-context/0.1",
                "context_digest": "c" * 64,
                "packet_digest": "p" * 64,
                "bundle_digest": "f" * 64,
                "generated_at": "2026-08-30T00:00:00Z",
                "query": args.get("query", {}),
                "context_text": "# AURORA PUBLIC-NEUROSURGICAL LITERATURE REASONING CONTEXT",
                "citations": [
                    {
                        "specialty": "chiari_malformation",
                        "pmid": "12345678",
                        "title": "A bounded Chiari citation",
                        "source_id": "pubmed_chiari",
                        "source_uri": "https://pubmed.ncbi.nlm.nih.gov/12345678/",
                        "record_uri": "https://pubmed.ncbi.nlm.nih.gov/12345678/",
                        "abstract_included": False,
                    }
                ],
                "included_citation_count": 1,
                "omitted_citation_count": 0,
                "context_char_count": 60,
                "truncated": False,
                "provenance_bound": True,
                "synthetic_data": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": [],
            }
        elif name == "neurosurgery_public_literature_matrix":
            payload = {
                "schema_version": "bioprism-neurosurgery-public-literature-matrix/0.1",
                "matrix_digest": "m" * 64,
                "bundle_digest": "f" * 64,
                "generated_at": "2026-08-30T00:00:00Z",
                "query": args.get("query", {}),
                "lanes": [
                    {"specialty": "glioma", "packet": {"packet_digest": "p" * 64}},
                    {"specialty": "chiari_malformation", "packet": {"packet_digest": "p" * 64}},
                ],
                "specialty_count": 2,
                "non_empty_lane_count": 2,
                "empty_lane_specialties": [],
                "total_match_count": 2,
                "total_returned_count": 2,
                "truncated_lane_count": 0,
                "returned_abstract_count": 2,
                "returned_without_abstract_count": 0,
                "provenance_bound": True,
                "synthetic_data": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": [],
            }
        elif name == "neurosurgery_real_data_reasoning_context":
            payload = {
                "schema_version": "bioprism-neurosurgery-real-data-reasoning-context/0.1",
                "context_digest": "c" * 64,
                "packet_digest": "p" * 64,
                "bundle_digest": "f" * 64,
                "generated_at": "2026-08-30T00:00:00Z",
                "query": args.get("query", {}),
                "context_text": "# AURORA REAL-GLIOMA REASONING CONTEXT",
                "citations": [
                    {
                        "record_kind": "genomic_project",
                        "record_id": "TCGA-GBM",
                        "title": "TCGA-GBM",
                        "source_id": "gdc_tcga",
                        "source_uri": "https://portal.gdc.cancer.gov/projects/TCGA-GBM",
                        "abstract_included": False,
                    },
                    {
                        "record_kind": "clinical_trial",
                        "record_id": "NCT00000001",
                        "title": "A bounded clinical trial",
                        "source_id": "clinicaltrials_glioma",
                        "source_uri": "https://clinicaltrials.gov/study/NCT00000001",
                        "abstract_included": False,
                    },
                    {
                        "record_kind": "portal_molecular_profile",
                        "record_id": "profile-1",
                        "title": "A bounded molecular profile",
                        "source_id": "cbioportal_glioma",
                        "source_uri": "https://www.cbioportal.org/study/gbm_tcga",
                        "abstract_included": False,
                    },
                ],
                "included_citation_count": 3,
                "omitted_citation_count": 0,
                "context_char_count": 38,
                "truncated": False,
                "provenance_bound": True,
                "synthetic_data": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": [],
            }
        elif name == "neurosurgery_research_plan":
            payload = {
                "schema_version": "bioprism-neurosurgery-research-plan/0.1",
                "request_digest": "p" * 64,
                "specialty": args.get("request", {}).get("specialty", "encephalocele"),
                "audit": {
                    "schema_version": "bioprism-neurosurgery-evidence-audit/0.1",
                    "request_digest": "a" * 64,
                    "specialty": "encephalocele",
                    "required_observation_kinds": [],
                    "items": [],
                    "missing_required_kinds": [],
                    "coverage_complete": True,
                    "human_review_required": True,
                    "provider": "none",
                    "network": False,
                    "effect": "read_only",
                },
                "tasks": [],
                "candidate_count": 0,
                "omitted_task_count": 0,
                "truncated": False,
                "source_query_count": 0,
                "source_candidate_count": 0,
                "coverage_complete": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": ["caller_observations_required"],
            }
        elif name == "neurosurgery_evidence_acquisition":
            operation = args.get("operation", "compile")
            if operation == "start":
                payload = {
                    "schema_version": "bioprism-neurosurgery-evidence-acquisition-session/0.1",
                    "plan": {"schema_version": "bioprism-neurosurgery-evidence-acquisition/0.1"},
                    "session": {
                        "schema_version": "bioprism-neurosurgery-evidence-acquisition-session/0.1",
                        "session_id": "nsa-session-" + "e" * 16,
                        "plan_digest": "e" * 64,
                        "request_digest": "p" * 64,
                        "specialty": args.get("request", {}).get("specialty", "glioma"),
                        "next_sequence": 1,
                        "status": "planned",
                        "event_chain_digest": "c" * 64,
                        "events": [],
                    },
                    "human_review_required": True,
                    "provider": "none",
                    "network": False,
                    "effect": "read_only",
                }
            elif operation == "advance":
                session = dict(args.get("session", {}))
                session["next_sequence"] = int(session.get("next_sequence", 1)) + 1
                session["status"] = "awaiting_human_review"
                payload = {
                    "schema_version": "bioprism-neurosurgery-evidence-acquisition-execution/0.1",
                    "session": session,
                    "steps_executed": 1,
                    "complete": True,
                    "steps": [],
                    "human_review_required": True,
                    "provider": "none",
                    "network": False,
                    "effect": "read_only",
                    "limitations": [],
                }
            elif operation == "finish":
                payload = {
                    "schema_version": "bioprism-neurosurgery-evidence-acquisition-execution/0.1",
                    "plan_digest": "e" * 64,
                    "request_digest": "p" * 64,
                    "specialty": args.get("request", {}).get("specialty", "glioma"),
                    "steps_executed": 1,
                    "event_count": 1,
                    "event_chain_digest": "c" * 64,
                    "human_review_required": True,
                    "provider": "none",
                    "network": False,
                    "effect": "read_only",
                    "limitations": [],
                }
            else:
                payload = {
                    "schema_version": "bioprism-neurosurgery-evidence-acquisition/0.1",
                    "plan_digest": "e" * 64,
                    "request_digest": "p" * 64,
                    "specialty": args.get("request", {}).get("specialty", "glioma"),
                    "query": args.get("query", {}),
                    "audit": {},
                    "steps": [],
                    "candidate_step_count": 0,
                    "omitted_step_count": 0,
                    "truncated": False,
                    "source_query_count": 0,
                    "source_candidate_count": 0,
                    "required_sources": [],
                    "ready_for_local_replay": False,
                    "human_review_required": True,
                    "provider": "none",
                    "network": False,
                    "effect": "read_only",
                    "limitations": [],
                }
        elif name == "neurosurgery_research_brief":
            payload = {
                "schema_version": "bioprism-neurosurgery-research-brief/0.1",
                "brief_digest": "r" * 64,
                "request_digest": "q" * 64,
                "source": "real_glioma",
                "specialty": args.get("request", {}).get("specialty", "glioma"),
                "bundle_digest": "b" * 64,
                "generated_at": "2026-08-30T00:00:00Z",
                "query": args.get("query", {}),
                "topics": [],
                "topic_count": 0,
                "non_empty_topic_count": 0,
                "total_match_count": 0,
                "total_returned_count": 0,
                "cross_topic_record_count": 0,
                "source_query_truncated": False,
                "unknowns": [],
                "review_prompts": ["verify source scope"],
                "freshness": None,
                "provenance_bound": True,
                "synthetic_data": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": ["lexical extraction only"],
            }
        elif name == "neurosurgery_real_data_query":
            payload = {
                "schema_version": "bioprism-neurosurgery-real/0.1",
                "bundle_digest": "e" * 64,
                "query": args["query"],
                "total_matches": 1,
                "returned_matches": 1,
                "truncated": False,
                "hits": [{"record_id": "NCT00000001"}],
            }
        elif name == "neurosurgery_real_data_trial_landscape":
            payload = {
                "schema_version": "bioprism-neurosurgery-real-data-trial-landscape/0.1",
                "landscape_digest": "l" * 64,
                "bundle_digest": "e" * 64,
                "generated_at": "2026-08-30T05:16:19Z",
                "query": args["query"],
                "total_matching_trials": 2,
                "returned_trial_count": 2,
                "omitted_trial_count": 0,
                "truncated": False,
                "status_counts": [{"label": "RECRUITING", "count": 2}],
                "phase_counts": [{"label": "PHASE2", "count": 2}],
                "phase_annotated_trial_count": 2,
                "study_type_counts": [{"label": "INTERVENTIONAL", "count": 2}],
                "intervention_counts": [{"name": "temozolomide", "count": 2}],
                "distinct_intervention_count": 1,
                "omitted_intervention_count": 0,
                "intervention_truncated": False,
                "missing_phase_count": 0,
                "missing_last_update_count": 0,
                "missing_study_type_count": 0,
                "missing_enrollment_count": 0,
                "missing_intervention_count": 0,
                "earliest_last_update": "2023-01-01",
                "latest_last_update": "2024-12-31",
                "source_ids": ["clinicaltrials_glioblastoma"],
                "review_reasons": [],
                "provenance_bound": True,
                "synthetic_data": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": ["metadata only"],
            }
        elif name == "neurosurgery_real_data_molecular_coverage":
            payload = {
                "schema_version": "bioprism-neurosurgery-real-data-molecular-coverage/0.1",
                "coverage_digest": "m" * 64,
                "bundle_digest": "e" * 64,
                "generated_at": "2026-08-30T05:16:19Z",
                "query": args["query"],
                "total_matching_profile_count": 6,
                "returned_profile_count": 6,
                "omitted_profile_count": 0,
                "truncated": False,
                "distinct_returned_study_count": 6,
                "emitted_study_count": 6,
                "omitted_study_count": 0,
                "study_rows_truncated": False,
                "emitted_profile_count": 6,
                "study_rows": [],
                "alteration_type_counts": [{"label": "MUTATION_EXTENDED", "count": 6}],
                "datatype_counts": [{"label": "MAF", "count": 6}],
                "patient_level_profile_count": 0,
                "analysis_visible_profile_count": 6,
                "description_present_count": 6,
                "missing_description_count": 0,
                "missing_alteration_type_count": 0,
                "missing_datatype_count": 0,
                "missing_study_link_count": 0,
                "source_ids": ["cbioportal_gbm_catalog"],
                "review_reasons": [],
                "provenance_bound": True,
                "synthetic_data": False,
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effect": "read_only",
                "limitations": ["metadata only"],
            }
        elif name == "neurosurgery_public_literature_query":
            payload = {
                "schema_version": "bioprism-neurosurgery-public-literature/0.1",
                "bundle_digest": "f" * 64,
                "query": args["query"],
                "total_matches": 1,
                "returned_matches": 1,
                "truncated": False,
                "hits": [{"specialty": "chiari_malformation", "pmid": "1"}],
            }
        elif name == "neurosurgery_mission":
            if args.get("operation") == "validate":
                payload = {
                    "valid": True,
                    "mission_id": "neurosurgical-mission-test",
                    "specialty": "glioma",
                    "status": "ready_for_human_review",
                    "human_review_required": True,
                    "request_digest": "r" * 64,
                    "audit_digest": "a" * 64,
                    "provider": "none",
                    "network": False,
                }
            else:
                payload = {
                "schema": "bioprism-neurosurgical-research-mission/0.1",
                "mission_id": "neurosurgical-mission-test",
                "specialty": "glioma",
                "status": "ready_for_human_review",
                "human_review_required": True,
                "provider": "none",
                "network": False,
                "effects": ["read_only"],
                "catalogue": {"specialty_count": 6, "tool_count": 16},
                "real_data_query": {"returned_matches": 1},
                "real_data_review_queue": {"bundle_digest": "b" * 64, "provider": "none", "network": False, "human_review_required": True},
                "real_data_evidence_packet": {"bundle_digest": "b" * 64, "provider": "none", "network": False, "human_review_required": True},
                "public_literature_evidence_packet": {"bundle_digest": "p" * 64, "provider": "none", "network": False, "human_review_required": True},
                "public_literature_integrity_audit": {"schema_version": "bioprism-neurosurgery-public-literature-integrity-audit/0.1", "bundle_digest": "p" * 64, "requires_integrity_review": True, "provider": "none", "network": False, "human_review_required": True},
                "public_literature_review_queue": {"schema_version": "bioprism-neurosurgery-public-literature-review-queue/0.1", "bundle_digest": "p" * 64, "candidate_item_count": 3, "returned_item_count": 3, "provider": "none", "network": False, "human_review_required": True},
                "public_literature_workbench": {"schema_version": "bioprism-neurosurgery-public-literature-workbench/0.1", "bundle_digest": "p" * 64, "specialty_count": 1, "total_record_count": 25, "provider": "none", "network": False, "synthetic_data": False, "human_review_required": True},
                "public_literature_portfolio": {"schema_version": "bioprism-neurosurgery-public-literature-portfolio/0.1", "bundle_digest": "p" * 64, "specialty_count": 2, "total_match_count": 48, "provider": "none", "network": False, "synthetic_data": False, "human_review_required": True},
                "literature_link_audit": {"schema_version": "bioprism-neurosurgery-literature-link-audit/0.1", "provider": "none", "network": False, "synthetic_data": False, "human_review_required": True},
                "real_data_evidence_graph": {"total_node_count": 88, "specialty": "glioma", "provider": "none", "network": False},
                "real_data_reasoning_context": {"context_digest": "c" * 64, "bundle_digest": "b" * 64, "synthetic_data": False, "network": False, "human_review_required": True, "context_text": "# AURORA REAL-GLIOMA REASONING CONTEXT"},
                "research_plan": {"schema_version": "bioprism-neurosurgery-research-plan/0.1", "request_digest": "p" * 64, "specialty": "glioma", "tasks": [], "human_review_required": True, "provider": "none", "network": False, "effect": "read_only", "limitations": []},
                "research_brief": {"schema_version": "bioprism-neurosurgery-research-brief/0.1", "source": "real_glioma", "specialty": "glioma", "brief_digest": "r" * 64, "provider": "none", "network": False, "human_review_required": True},
                    "run": {"steps_executed": 2},
                }
        elif name == "neurosurgery_session":
            operation = args["operation"]
            if operation == "start":
                payload = {
                    "schema_version": "bioprism-neurosurgery/0.1",
                    "session_id": "ns-session-test",
                    "request_digest": "a" * 64,
                    "specialty": "glioma",
                    "route": ["safety_gate", "human_review_hold"],
                    "next_ordinal": 1,
                    "status": "planned",
                    "event_chain_digest": "b" * 64,
                    "events": [],
                }
            elif operation == "advance":
                prior = args["session"]
                ordinal = len(prior["events"]) + 1
                capability = prior["route"][ordinal - 1]
                event = {
                    "ordinal": ordinal,
                    "capability": capability,
                    "status": "held_for_human_review" if capability == "human_review_hold" else "completed",
                    "finding_digest": "c" * 64,
                    "previous_event_digest": prior["event_chain_digest"],
                    "event_digest": "d" * 64,
                }
                payload = dict(prior)
                payload["events"] = [*prior["events"], event]
                payload["next_ordinal"] = ordinal + 1
                payload["status"] = "awaiting_human_review" if capability == "human_review_hold" else "running"
                payload["event_chain_digest"] = event["event_digest"]
            elif operation == "finish":
                payload = {"status": "ready_for_human_review", "plan": [], "tool_runs": []}
            elif operation == "run":
                payload = {
                    "schema_version": "bioprism-neurosurgery/0.1",
                    "steps_executed": 2,
                    "session": {"status": "awaiting_human_review", "next_ordinal": 3, "route": ["safety_gate", "human_review_hold"]},
                    "response": {"status": "ready_for_human_review", "plan": [], "tool_runs": []},
                }
            else:  # pragma: no cover - the facade only emits known operations
                raise AssertionError(operation)
        else:  # pragma: no cover - test fixture guard
            raise AssertionError(name)
        return ToolResult(
            tool=name,
            envelope={"content": [{"type": "text", "text": json.dumps(payload)}]},
        )


class LifecycleClient(FakeClient):
    def __init__(self) -> None:
        super().__init__()
        self.connected = False
        self.closed = False

    def connect(self) -> None:
        self.connected = True

    def close(self) -> None:
        self.closed = True


class NeurosurgeryFacadeTests(unittest.TestCase):
    def test_neurosurgical_tools_are_curated_for_autonomous_biomedical_routes(self) -> None:
        profiles = {profile.domain: profile for profile in builtin_autonomous_domain_tool_profiles()}
        expected = {
            "neurosurgery_catalogue",
            "neurosurgery_intake_plan",
            "neurosurgery_intake_mission",
            "neurosurgery_intake_portfolio",
            "neurosurgery_evidence_audit",
            "neurosurgery_specialty_evidence_map",
            "neurosurgery_case_asset_manifest",
            "neurosurgery_case_fhir_import",
            "neurosurgery_case_asset_review_disposition",
            "neurosurgery_case_dicom_import",
            "neurosurgery_case_dicom_evidence_workflow",
            "neurosurgery_evidence_synthesis",
            "neurosurgery_evidence_graph",
            "neurosurgery_glioma_molecular_map",
            "neurosurgery_real_data_coverage",
            "neurosurgery_real_data_reconciliation",
            "neurosurgery_real_data_freshness",
            "neurosurgery_real_data_diff",
            "neurosurgery_real_data_refresh_audit",
            "neurosurgery_real_data_review_queue",
            "neurosurgery_real_data_review_disposition",
            "neurosurgery_real_data_evidence_packet",
            "neurosurgery_real_data_autonomous_workflow",
            "neurosurgery_real_data_reasoning_context",
            "neurosurgery_real_data_draft_audit",
            "neurosurgery_public_literature_evidence_packet",
            "neurosurgery_public_literature_reasoning_context",
            "neurosurgery_public_literature_draft_audit",
            "neurosurgery_public_literature_matrix",
            "neurosurgery_public_literature_freshness",
            "neurosurgery_public_literature_refresh_audit",
            "neurosurgery_literature_link_audit",
            "neurosurgery_public_literature_integrity_audit",
            "neurosurgery_public_literature_review_queue",
            "neurosurgery_public_literature_workbench",
            "neurosurgery_public_literature_portfolio",
            "neurosurgery_evidence_program",
            "neurosurgery_research_brief",
            "neurosurgery_research_plan",
            "neurosurgery_evidence_acquisition",
            "neurosurgery_plan",
            "neurosurgery_real_data_query",
            "neurosurgery_real_data_trial_landscape",
            "neurosurgery_real_data_molecular_coverage",
            "neurosurgery_public_literature_query",
            "neurosurgery_session",
            "neurosurgery_mission",
        }
        for domain in ("biomedical", "neuroscience"):
            bindings = profiles[domain].bindings
            self.assertEqual(
                {binding.name for binding in bindings if binding.name.startswith("neurosurgery_")},
                expected,
            )
            self.assertTrue(
                all(binding.read_only and not binding.approval_required for binding in bindings if binding.name in expected)
            )

    def test_catalogue_plan_and_session_lifecycle_are_composed(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        self.assertEqual(
            [tool["name"] for tool in agent.catalogue()],
            [
                "neurosurgery_plan",
                "neurosurgery_session",
                "neurosurgery_catalogue",
                "neurosurgery_intake_plan",
                "neurosurgery_intake_mission",
                "neurosurgery_intake_portfolio",
                "neurosurgery_evidence_audit",
                "neurosurgery_specialty_evidence_map",
                "neurosurgery_case_asset_manifest",
                "neurosurgery_case_fhir_import",
                "neurosurgery_case_dicom_import",
                "neurosurgery_case_dicom_evidence_workflow",
                "neurosurgery_case_asset_review_disposition",
                "neurosurgery_evidence_synthesis",
                "neurosurgery_evidence_graph",
                "neurosurgery_real_data_coverage",
                "neurosurgery_real_data_reconciliation",
                "neurosurgery_real_data_freshness",
                "neurosurgery_real_data_diff",
                "neurosurgery_real_data_refresh_audit",
                "neurosurgery_real_data_review_queue",
                "neurosurgery_real_data_review_disposition",
                "neurosurgery_real_data_evidence_packet",
                "neurosurgery_real_data_reasoning_context",
                "neurosurgery_real_data_draft_audit",
                "neurosurgery_public_literature_evidence_packet",
                "neurosurgery_public_literature_reasoning_context",
                "neurosurgery_public_literature_draft_audit",
                "neurosurgery_public_literature_matrix",
                "neurosurgery_public_literature_freshness",
                "neurosurgery_public_literature_refresh_audit",
                "neurosurgery_literature_link_audit",
                "neurosurgery_public_literature_integrity_audit",
                "neurosurgery_public_literature_review_queue",
                "neurosurgery_public_literature_workbench",
                "neurosurgery_public_literature_portfolio",
                "neurosurgery_research_brief",
                "neurosurgery_research_plan",
                "neurosurgery_evidence_acquisition",
                "neurosurgery_real_data_query",
                "neurosurgery_real_data_trial_landscape",
                "neurosurgery_real_data_molecular_coverage",
                "neurosurgery_public_literature_query",
                "neurosurgery_mission",
            ],
        )
        catalogue = agent.specialty_catalogue()
        self.assertEqual(catalogue["provider"], "none")
        self.assertFalse(catalogue["network"])
        intake = agent.intake_plan("Review glioma MGMT and IDH evidence")
        self.assertEqual(intake["selected_specialty"], "glioma")
        self.assertFalse(intake["abstained"])
        self.assertEqual(intake["provider"], "none")
        self.assertNotIn("question", intake)
        self.assertEqual(client.calls[-1][0], "neurosurgery_intake_plan")
        mission = agent.intake_mission(
            "Review glioma MGMT and IDH evidence",
            real_glioma_data={"schema_version": "bioprism-neurosurgery-real/0.1"},
            case_asset_manifest={
                "schema_version": "bioprism-neurosurgery-case-asset-manifest/0.1",
                "specialty": "glioma",
                "synthetic_data": False,
                "assets": [],
            },
            case_asset_manifest_query={"requested_kinds": ["imaging_series"]},
            case_asset_review_disposition={"report_digest": "d" * 64},
            freshness={"as_of": "2027-08-31T00:00:00Z", "max_age_days": 30},
            max_session_steps=32,
        )
        self.assertEqual(mission["status"], "ready_for_human_review")
        self.assertEqual(mission["provider"], "none")
        self.assertNotIn("question", mission)
        self.assertEqual(
            client.calls[-1][1]["case_asset_review_disposition"],
            {"report_digest": "d" * 64},
        )
        self.assertEqual(client.calls[-1][0], "neurosurgery_intake_mission")
        self.assertEqual(client.calls[-1][1]["max_session_steps"], 32)
        self.assertEqual(
            client.calls[-1][1]["case_asset_manifest_query"]["requested_kinds"],
            ["imaging_series"],
        )
        self.assertEqual(client.calls[-1][1]["freshness"]["max_age_days"], 30)
        imported_mission = agent.intake_mission(
            "Review glioma imaging and molecular evidence",
            real_glioma_data={"schema_version": "bioprism-neurosurgery-real/0.1"},
            case_dicom_import={"schema_version": "bioprism-neurosurgery-case-dicom-import/0.1"},
            case_fhir_import={"schema_version": "bioprism-neurosurgery-case-fhir-import/0.1"},
            max_session_steps=32,
        )
        self.assertEqual(imported_mission["status"], "ready_for_human_review")
        self.assertIn("case_dicom_import", client.calls[-1][1])
        self.assertIn("case_fhir_import", client.calls[-1][1])
        with self.assertRaises(ArgumentError):
            agent.intake_mission(
                "Review glioma evidence",
                case_asset_manifest={"schema_version": "bioprism-neurosurgery-case-asset-manifest/0.1"},
                case_dicom_import={"schema_version": "bioprism-neurosurgery-case-dicom-import/0.1"},
            )
        with self.assertRaises(ArgumentError):
            agent.intake_mission(
                "Review glioma evidence",
                case_asset_manifest_query={"requested_kinds": ["imaging_series"]},
            )
        selected_portfolio = agent.intake_portfolio(
            "Review glioma evidence",
            specialty="glioma",
            real_glioma_data={"schema_version": "bioprism-neurosurgery-real/0.1"},
            public_literature={
                "schema_version": "bioprism-neurosurgery-public-literature/0.1"
            },
            case_asset_manifest={
                "schema_version": "bioprism-neurosurgery-case-asset-manifest/0.1",
                "specialty": "glioma",
                "synthetic_data": False,
                "assets": [],
            },
            case_asset_manifest_query={"requested_kinds": ["pathology_report"]},
            case_asset_review_disposition={"report_digest": "d" * 64, "decisions": []},
            freshness={"as_of": "2027-08-31T00:00:00Z"},
            max_session_steps=16,
        )
        self.assertEqual(selected_portfolio["status"], "ready_for_human_review")
        self.assertEqual(
            client.calls[-1][1]["case_asset_manifest_query"]["requested_kinds"],
            ["pathology_report"],
        )
        self.assertEqual(
            client.calls[-1][1]["case_asset_review_disposition"]["report_digest"],
            "d" * 64,
        )
        self.assertEqual(client.calls[-1][1]["freshness"]["max_age_days"], 365)
        with self.assertRaises(ArgumentError):
            agent.intake_portfolio(
                "Review glioma evidence",
                case_asset_review_disposition={"report_digest": "d" * 64},
            )
        portfolio = agent.intake_portfolio(
            "Review all neurosurgical evidence lanes",
            include_all_specialties=True,
            public_literature={"schema_version": "bioprism-neurosurgery-public-literature/0.1"},
            max_hits_per_lane=4,
            max_review_items_per_lane=4,
            max_issues_per_lane=8,
            max_session_steps=16,
        )
        self.assertEqual(portfolio["status"], "ready_for_human_review")
        self.assertEqual(len(portfolio["selected_specialties"]), 6)
        self.assertEqual(client.calls[-1][0], "neurosurgery_intake_portfolio")
        self.assertTrue(client.calls[-1][1]["include_all_specialties"])
        with self.assertRaises(ArgumentError):
            agent.intake_plan("   ")
        with self.assertRaises(ArgumentError):
            agent.intake_mission("Review glioma evidence", freshness={"as_of": "2026-02-30T00:00:00Z"})
        audit = agent.audit_evidence({"specialty": "encephalocele", "request_use": "research_synthesis"})
        self.assertFalse(audit["coverage_complete"])
        self.assertEqual(client.calls[-1][0], "neurosurgery_evidence_audit")
        specialty_map = agent.specialty_evidence_map(
            {"specialty": "glioma", "request_use": "research_synthesis"}
        )
        self.assertEqual(
            specialty_map["schema_version"],
            "bioprism-neurosurgery-specialty-evidence-map/0.1",
        )
        self.assertEqual(client.calls[-1][0], "neurosurgery_specialty_evidence_map")
        self.assertEqual(client.calls[-1][1]["request"]["specialty"], "glioma")
        asset_manifest = agent.case_asset_manifest(
            {"specialty": "glioma", "request_use": "research_synthesis"},
            {
                "schema_version": "bioprism-neurosurgery-case-asset-manifest/0.1",
                "specialty": "glioma",
                "synthetic_data": False,
                "assets": [],
            },
            requested_kinds=["imaging_series"],
            max_review_items=16,
        )
        self.assertEqual(
            asset_manifest["schema_version"],
            "bioprism-neurosurgery-case-asset-manifest/0.1",
        )
        self.assertEqual(asset_manifest["provider"], "none")
        self.assertEqual(client.calls[-1][0], "neurosurgery_case_asset_manifest")
        self.assertEqual(client.calls[-1][1]["query"]["requested_kinds"], ["imaging_series"])
        disposition = agent.case_asset_review_disposition(
            asset_manifest,
            [{"sequence": 1, "disposition": "reviewed", "reviewer_id": "reviewer-a"}],
        )
        self.assertEqual(
            disposition["schema_version"],
            "bioprism-neurosurgery-case-asset-review-disposition/0.1",
        )
        self.assertEqual(client.calls[-1][0], "neurosurgery_case_asset_review_disposition")
        self.assertEqual(
            client.calls[-1][1]["decisions"],
            [{"sequence": 1, "disposition": "reviewed", "reviewer_id": "reviewer-a"}],
        )
        fhir_report = agent.case_fhir_import(
            {"specialty": "glioma", "request_use": "research_synthesis"},
            {
                "schema_version": "bioprism-neurosurgery-case-fhir-import/0.1",
                "specialty": "glioma",
                "deidentified": True,
                "synthetic_data": False,
                "source_id": "export-a",
                "bundle": {"resourceType": "Bundle", "entry": []},
            },
        )
        self.assertEqual(
            fhir_report["schema_version"],
            "bioprism-neurosurgery-case-fhir-import/0.1",
        )
        self.assertEqual(client.calls[-1][0], "neurosurgery_case_fhir_import")
        self.assertEqual(client.calls[-1][1]["import"]["source_id"], "export-a")
        dicom_report = agent.case_dicom_import(
            {"specialty": "glioma", "request_use": "research_synthesis"},
            {
                "schema_version": "bioprism-neurosurgery-case-dicom-import/0.1",
                "specialty": "glioma",
                "deidentified": True,
                "synthetic_data": False,
                "source_id": "dicom-export-a",
                "datasets": [],
            },
        )
        self.assertEqual(
            dicom_report["schema_version"],
            "bioprism-neurosurgery-case-dicom-import/0.1",
        )
        self.assertEqual(client.calls[-1][0], "neurosurgery_case_dicom_import")
        self.assertEqual(client.calls[-1][1]["import"]["source_id"], "dicom-export-a")
        workflow = agent.case_dicom_evidence_workflow(
            {"specialty": "glioma", "request_use": "research_synthesis"},
            {"schema_version": "bioprism-neurosurgery-case-dicom-import/0.1", "specialty": "glioma", "datasets": []},
            real_glioma_data={"schema_version": "bioprism-neurosurgery-real/0.1"},
            query={
                "max_acquisition_steps": 4,
                "real_data_reasoning_context": {"max_chars": 10000},
            },
        )
        self.assertEqual(
            workflow["schema_version"],
            "bioprism-neurosurgery-case-dicom-evidence-workflow/0.1",
        )
        self.assertEqual(client.calls[-1][0], "neurosurgery_case_dicom_evidence_workflow")
        self.assertEqual(client.calls[-1][1]["query"]["max_acquisition_steps"], 4)
        self.assertEqual(
            client.calls[-1][1]["query"]["real_data_reasoning_context"]["max_chars"],
            10000,
        )
        with self.assertRaises(ArgumentError):
            agent.case_asset_manifest(
                {"specialty": "glioma", "request_use": "research_synthesis"},
                {
                    "schema_version": "bioprism-neurosurgery-case-asset-manifest/0.1",
                    "specialty": "glioma",
                    "assets": [],
                },
                requested_kinds=["imaging_series", "imaging_series"],
            )
        synthesis = agent.evidence_synthesis(
            {"specialty": "glioma", "request_use": "research_synthesis"},
            real_glioma_data={"schema_version": "bioprism-neurosurgery-real/0.1"},
            public_literature={"schema_version": "bioprism-neurosurgery-public-literature/0.1"},
            query={"max_references": 12, "include_source_text": True},
        )
        self.assertEqual(synthesis["schema_version"], "bioprism-neurosurgery-evidence-synthesis/0.1")
        self.assertFalse(synthesis["synthetic_data"])
        self.assertEqual(client.calls[-1][0], "neurosurgery_evidence_synthesis")
        self.assertEqual(client.calls[-1][1]["query"]["max_references"], 12)
        with self.assertRaises(ArgumentError):
            agent.evidence_synthesis(
                {"specialty": "glioma", "request_use": "research_synthesis"},
                case_asset_manifest_query={"requested_kinds": ["imaging_series"]},
            )
        direct_manifest = {
            "schema_version": "bioprism-neurosurgery-case-asset-manifest/0.1",
            "specialty": "glioma",
            "synthetic_data": False,
            "assets": [],
        }
        agent.evidence_synthesis(
            {"specialty": "glioma", "request_use": "research_synthesis"},
            real_glioma_data={"schema_version": "bioprism-neurosurgery-real/0.1"},
            case_asset_manifest=direct_manifest,
            case_asset_manifest_query={"requested_kinds": ["imaging_series"]},
        )
        self.assertEqual(client.calls[-1][1]["case_asset_manifest"], direct_manifest)
        self.assertEqual(
            client.calls[-1][1]["case_asset_manifest_query"]["requested_kinds"],
            ["imaging_series"],
        )
        disposition_ledger = {
            "schema_version": "bioprism-neurosurgery-case-asset-review-disposition/0.1",
            "report_digest": "d" * 64,
            "disposition_digest": "x" * 64,
            "candidate_item_count": 0,
            "returned_item_count": 0,
            "omitted_item_count": 0,
            "submitted_decision_count": 0,
            "accepted_decision_count": 0,
            "resolved_decision_count": 0,
            "unresolved_decision_count": 0,
            "undecided_returned_item_count": 0,
            "pending_item_count": 0,
            "decisions": [],
            "unresolved_sequences": [],
            "undecided_sequences": [],
            "provenance_bound": True,
            "synthetic_data": False,
            "human_review_required": True,
            "provider": "none",
            "network": False,
            "effect": "read_only",
            "limitations": [],
        }
        agent.evidence_synthesis(
            {"specialty": "glioma", "request_use": "research_synthesis"},
            case_asset_manifest=direct_manifest,
            case_asset_review_disposition=disposition_ledger,
        )
        self.assertEqual(
            client.calls[-1][1]["case_asset_review_disposition"], disposition_ledger
        )
        temporal = agent.temporal_audit({"specialty": "encephalocele", "request_use": "research_synthesis"})
        self.assertEqual(temporal["schema_version"], "bioprism-neurosurgery-temporal-alignment/0.1")
        self.assertEqual(temporal["status"], "unavailable")
        research_plan = agent.plan_research(
            {"specialty": "encephalocele", "request_use": "research_synthesis"},
            max_tasks=3,
            max_references_per_task=2,
        )
        self.assertEqual(research_plan["schema_version"], "bioprism-neurosurgery-research-plan/0.1")
        self.assertTrue(research_plan["human_review_required"])
        self.assertEqual(client.calls[-1][0], "neurosurgery_research_plan")
        acquisition = agent.evidence_acquisition(
            {"specialty": "glioma", "request_use": "research_synthesis"},
            real_glioma_data={"schema_version": "bioprism-neurosurgery-real/0.1"},
            public_literature={"schema_version": "bioprism-neurosurgery-public-literature/0.1"},
            query={
                "max_steps": 8,
                "max_references_per_step": 2,
                "freshness": {"as_of": "2026-08-30T00:00:00Z", "max_age_days": 30},
            },
        )
        self.assertEqual(acquisition["schema_version"], "bioprism-neurosurgery-evidence-acquisition/0.1")
        self.assertEqual(client.calls[-1][0], "neurosurgery_evidence_acquisition")
        self.assertEqual(client.calls[-1][1]["query"]["max_steps"], 8)
        self.assertEqual(client.calls[-1][1]["query"]["freshness"]["max_age_days"], 30)
        acquisition_manifest = {
            "schema_version": "bioprism-neurosurgery-case-asset-manifest/0.1",
            "specialty": "glioma",
            "synthetic_data": False,
            "assets": [],
        }
        agent.evidence_acquisition(
            {"specialty": "glioma", "request_use": "research_synthesis"},
            real_glioma_data={"schema_version": "bioprism-neurosurgery-real/0.1"},
            case_asset_manifest=acquisition_manifest,
            case_asset_manifest_query={"requested_kinds": ["imaging_series"]},
        )
        self.assertEqual(
            client.calls[-1][1]["case_asset_manifest"],
            acquisition_manifest,
        )
        self.assertEqual(
            client.calls[-1][1]["case_asset_manifest_query"]["requested_kinds"],
            ["imaging_series"],
        )
        with self.assertRaises(ArgumentError):
            agent.evidence_acquisition(
                {"specialty": "glioma", "request_use": "research_synthesis"},
                query={"max_steps": 0},
            )
        started = agent.evidence_acquisition_start(
            {"specialty": "glioma", "request_use": "research_synthesis"},
            real_glioma_data={"schema_version": "bioprism-neurosurgery-real/0.1"},
            query={"max_steps": 2},
        )
        self.assertEqual(
            started["schema_version"],
            "bioprism-neurosurgery-evidence-acquisition-session/0.1",
        )
        advanced = agent.evidence_acquisition_advance(
            {"specialty": "glioma", "request_use": "research_synthesis"},
            started["session"],
            real_glioma_data={"schema_version": "bioprism-neurosurgery-real/0.1"},
            query={"max_steps": 2},
            max_steps=1,
        )
        self.assertTrue(advanced["complete"])
        self.assertEqual(client.calls[-1][1]["operation"], "advance")
        finished = agent.evidence_acquisition_finish(
            {"specialty": "glioma", "request_use": "research_synthesis"},
            advanced["session"],
            real_glioma_data={"schema_version": "bioprism-neurosurgery-real/0.1"},
            query={"max_steps": 2},
        )
        self.assertEqual(
            finished["schema_version"],
            "bioprism-neurosurgery-evidence-acquisition-execution/0.1",
        )
        research_brief = agent.research_brief(
            {"specialty": "glioma", "request_use": "research_synthesis"},
            real_glioma_data={"schema_version": "bioprism-neurosurgery-real/0.1", "synthetic_data": False},
            query={"focus_terms": ["glioblastoma"], "max_topics": 4, "max_records_per_topic": 2, "include_abstracts": True},
        )
        self.assertEqual(research_brief["schema_version"], "bioprism-neurosurgery-research-brief/0.1")
        self.assertEqual(research_brief["source"], "real_glioma")
        self.assertEqual(research_brief["provider"], "none")
        self.assertEqual(client.calls[-1][0], "neurosurgery_research_brief")
        self.assertEqual(client.calls[-1][1]["query"]["focus_terms"], ["glioblastoma"])
        with self.assertRaises(ArgumentError):
            agent.research_brief({"specialty": "glioma", "request_use": "research_synthesis"})
        graph = agent.evidence_graph(
            {"schema_version": "bioprism-neurosurgery-real/0.1"},
            root_record_id="24120142",
            root_record_kind="literature_article",
            max_nodes=16,
            max_edges=32,
        )
        self.assertEqual(graph["schema_version"], "bioprism-neurosurgery-evidence-graph/0.1")
        self.assertTrue(graph["human_review_required"])
        self.assertEqual(client.calls[-1][0], "neurosurgery_evidence_graph")
        self.assertEqual(client.calls[-1][1]["query"]["root_record_kind"], "literature_article")
        self.assertEqual(client.calls[-1][1]["query"]["max_edges"], 32)
        coverage = agent.real_data_coverage(
            {"schema_version": "bioprism-neurosurgery-real/0.1"},
            record_kind="clinical_trial",
            source_id="clinicaltrials_glioma_2026-08-30",
            from_year=2020,
            to_year=2025,
        )
        self.assertEqual(coverage["schema_version"], "bioprism-neurosurgery-real-data-coverage/0.1")
        self.assertEqual(coverage["matched_record_count"], 4)
        self.assertFalse(coverage["synthetic_data"])
        self.assertEqual(client.calls[-1][0], "neurosurgery_real_data_coverage")
        self.assertEqual(client.calls[-1][1]["query"]["from_year"], 2020)
        reconciliation = agent.real_data_reconciliation(
            {"schema_version": "bioprism-neurosurgery-real/0.1"},
            max_issues=12,
        )
        self.assertEqual(
            reconciliation["schema_version"],
            "bioprism-neurosurgery-real-data-reconciliation/0.1",
        )
        self.assertFalse(reconciliation["synthetic_data"])
        self.assertEqual(reconciliation["provider"], "none")
        self.assertEqual(client.calls[-1][0], "neurosurgery_real_data_reconciliation")
        self.assertEqual(client.calls[-1][1]["query"]["max_issues"], 12)
        with self.assertRaises(ArgumentError):
            agent.real_data_reconciliation(
                {"schema_version": "bioprism-neurosurgery-real/0.1"},
                max_issues=0,
            )
        freshness = agent.real_data_freshness(
            {"schema_version": "bioprism-neurosurgery-real/0.1"},
            as_of="2026-08-31T00:00:00Z",
            max_age_days=30,
        )
        self.assertEqual(freshness["schema_version"], "bioprism-neurosurgery-real-data-freshness/0.1")
        self.assertEqual(freshness["status"], "stale")
        self.assertEqual(client.calls[-1][0], "neurosurgery_real_data_freshness")
        self.assertEqual(client.calls[-1][1]["query"]["max_age_days"], 30)
        with self.assertRaises(ArgumentError):
            agent.real_data_freshness(
                {"schema_version": "bioprism-neurosurgery-real/0.1"},
                as_of="2026-02-30T00:00:00Z",
            )
        diff = agent.real_data_diff(
            {"schema_version": "bioprism-neurosurgery-real/0.1"},
            {"schema_version": "bioprism-neurosurgery-real/0.1"},
            record_kind="clinical_trial",
            source_id="clinicaltrials_glioma_2026-08-30",
            max_changes=4,
        )
        self.assertEqual(diff["schema_version"], "bioprism-neurosurgery-real-data-diff/0.1")
        self.assertEqual(diff["record_counts"]["changed"], 1)
        self.assertEqual(client.calls[-1][0], "neurosurgery_real_data_diff")
        self.assertEqual(client.calls[-1][1]["query"]["max_changes"], 4)
        refresh = agent.real_data_refresh_audit(
            {"specialty": "glioma", "request_use": "research_synthesis"},
            {"schema_version": "bioprism-neurosurgery-real/0.1", "synthetic_data": False},
            {"schema_version": "bioprism-neurosurgery-real/0.1", "synthetic_data": False},
            query={"brief": {"focus_terms": ["MGMT"]}},
        )
        self.assertEqual(refresh["schema_version"], "bioprism-neurosurgery-real-data-refresh-audit/0.1")
        self.assertTrue(refresh["source_identity_stable"])
        self.assertEqual(refresh["provider"], "none")
        self.assertEqual(client.calls[-1][0], "neurosurgery_real_data_refresh_audit")
        self.assertEqual(client.calls[-1][1]["query"]["brief"]["focus_terms"], ["MGMT"])
        queue = agent.real_data_review_queue(
            {"schema_version": "bioprism-neurosurgery-real/0.1"},
            record_kind="portal_study",
            source_id="cbioportal_glioma_2026-08-30",
            max_items=2,
        )
        self.assertEqual(
            queue["schema_version"],
            "bioprism-neurosurgery-real-data-review-queue/0.1",
        )
        self.assertEqual(queue["candidate_item_count"], 15)
        self.assertEqual(client.calls[-1][0], "neurosurgery_real_data_review_queue")
        self.assertEqual(client.calls[-1][1]["query"]["max_items"], 2)
        disposition = agent.real_data_review_disposition(
            queue,
            [{"task_id": "review-task", "disposition": "reviewed", "reviewer_id": "py-test"}],
        )
        self.assertEqual(
            disposition["schema_version"],
            "bioprism-neurosurgery-real-data-review-disposition/0.1",
        )
        self.assertEqual(client.calls[-1][0], "neurosurgery_real_data_review_disposition")
        self.assertEqual(len(client.calls[-1][1]["decisions"]), 1)
        packet = agent.real_data_evidence_packet(
            {"schema_version": "bioprism-neurosurgery-real/0.1"},
            data_query={"text": "glioblastoma", "limit": 4},
            graph={"max_nodes": 8, "max_edges": 12},
            review_queue={"max_items": 3},
            freshness={"as_of": "2027-08-31T00:00:00Z", "max_age_days": 30},
        )
        self.assertEqual(
            packet["schema_version"],
            "bioprism-neurosurgery-real-data-evidence-packet/0.4",
        )
        self.assertEqual(client.calls[-1][0], "neurosurgery_real_data_evidence_packet")
        self.assertEqual(client.calls[-1][1]["query"]["query"]["limit"], 4)

        self.assertEqual(
            client.calls[-1][1]["query"]["freshness"]["as_of"], "2027-08-31T00:00:00Z"
        )
        context = agent.real_data_reasoning_context(
            {"schema_version": "bioprism-neurosurgery-real/0.1"},
            packet={"query": {"text": "glioblastoma", "limit": 2}},
            max_chars=6000,
            include_abstracts=True,
        )
        self.assertEqual(
            context["schema_version"],
            "bioprism-neurosurgery-real-data-reasoning-context/0.1",
        )
        self.assertEqual(client.calls[-1][0], "neurosurgery_real_data_reasoning_context")
        self.assertEqual(client.calls[-1][1]["query"]["packet"]["query"]["limit"], 2)
        self.assertEqual(client.calls[-1][1]["query"]["max_chars"], 6000)
        self.assertTrue(client.calls[-1][1]["query"]["include_abstracts"])
        literature_packet = agent.public_literature_evidence_packet(
            {"schema_version": "bioprism-neurosurgery-public-literature/0.1"},
            query={"specialty": "chiari_malformation", "limit": 2},
            freshness={"as_of": "2027-08-31T00:00:00Z", "max_age_days": 30},
        )
        self.assertEqual(
            literature_packet["schema_version"],
            "bioprism-neurosurgery-public-literature-evidence-packet/0.1",
        )
        self.assertEqual(client.calls[-1][0], "neurosurgery_public_literature_evidence_packet")
        self.assertEqual(client.calls[-1][1]["query"]["query"]["limit"], 2)
        self.assertEqual(
            client.calls[-1][1]["query"]["freshness"]["max_age_days"], 30
        )
        literature_freshness = agent.public_literature_freshness(
            {"schema_version": "bioprism-neurosurgery-public-literature/0.1"},
            as_of="2026-08-31T00:00:00Z",
        )
        self.assertEqual(literature_freshness["status"], "current")
        self.assertEqual(client.calls[-1][0], "neurosurgery_public_literature_freshness")
        literature_refresh = agent.public_literature_refresh_audit(
            {"schema_version": "bioprism-neurosurgery-public-literature/0.1", "synthetic_data": False},
            {"schema_version": "bioprism-neurosurgery-public-literature/0.1", "synthetic_data": False},
            query={"max_source_changes": 8, "max_record_changes": 16},
        )
        self.assertEqual(
            literature_refresh["schema_version"],
            "bioprism-neurosurgery-public-literature-refresh-audit/0.1",
        )
        self.assertEqual(client.calls[-1][0], "neurosurgery_public_literature_refresh_audit")
        self.assertEqual(client.calls[-1][1]["query"]["max_record_changes"], 16)
        literature_link = agent.literature_link_audit(
            {"schema_version": "bioprism-neurosurgery-real-glioma/0.1"},
            {"schema_version": "bioprism-neurosurgery-public-literature/0.1"},
            query={"max_links": 8, "max_unmatched_ids": 16},
        )
        self.assertEqual(
            literature_link["schema_version"],
            "bioprism-neurosurgery-literature-link-audit/0.1",
        )
        self.assertEqual(literature_link["counts"]["linked_real_records"], 12)
        self.assertEqual(client.calls[-1][0], "neurosurgery_literature_link_audit")
        self.assertEqual(client.calls[-1][1]["query"]["max_links"], 8)
        literature_integrity = agent.public_literature_integrity_audit(
            {"schema_version": "bioprism-neurosurgery-public-literature/0.1"},
            query={"specialties": ["glioma"], "max_issues": 8},
        )
        self.assertEqual(
            literature_integrity["schema_version"],
            "bioprism-neurosurgery-public-literature-integrity-audit/0.1",
        )
        self.assertEqual(literature_integrity["counts"]["missing_abstract_count"], 7)
        self.assertEqual(client.calls[-1][0], "neurosurgery_public_literature_integrity_audit")
        self.assertEqual(client.calls[-1][1]["query"]["max_issues"], 8)
        literature_queue = agent.public_literature_review_queue(
            {"schema_version": "bioprism-neurosurgery-public-literature/0.1"},
            specialties=["glioma"],
            max_items=8,
        )
        self.assertEqual(
            literature_queue["schema_version"],
            "bioprism-neurosurgery-public-literature-review-queue/0.1",
        )
        self.assertEqual(client.calls[-1][0], "neurosurgery_public_literature_review_queue")
        self.assertEqual(client.calls[-1][1]["query"]["max_items"], 8)
        workbench = agent.public_literature_workbench(
            {"schema_version": "bioprism-neurosurgery-public-literature/0.1"},
            specialties=["glioma"],
            max_issues_per_lane=8,
            freshness={"as_of": "2027-08-31T00:00:00Z", "max_age_days": 30},
        )
        self.assertEqual(
            workbench["schema_version"],
            "bioprism-neurosurgery-public-literature-workbench/0.1",
        )
        self.assertEqual(workbench["lanes"][0]["profile"]["specialty"], "glioma")
        self.assertEqual(workbench["total_record_count"], 25)
        self.assertEqual(client.calls[-1][0], "neurosurgery_public_literature_workbench")
        self.assertEqual(client.calls[-1][1]["query"]["max_issues_per_lane"], 8)
        self.assertEqual(client.calls[-1][1]["query"]["freshness"]["max_age_days"], 30)
        with self.assertRaises(ArgumentError):
            agent.public_literature_workbench({}, specialties=["glioma", "glioma"])
        with self.assertRaises(ArgumentError):
            agent.public_literature_workbench({}, max_issues_per_lane=1.5)  # type: ignore[arg-type]
        with self.assertRaises(ArgumentError):
            agent.public_literature_workbench({}, freshness={"as_of": "2026-02-30T00:00:00Z"})
        portfolio = agent.public_literature_portfolio(
            {"schema_version": "bioprism-neurosurgery-public-literature/0.1"},
            specialties=["glioma", "chiari_malformation"],
            text="glioblastoma",
            from_date="2020-01-01",
            to_date="2026-01-01",
            max_hits_per_lane=2,
            max_review_items_per_lane=2,
            max_issues_per_lane=8,
            freshness={"as_of": "2027-08-31T00:00:00Z", "max_age_days": 30},
        )
        self.assertEqual(
            portfolio["schema_version"],
            "bioprism-neurosurgery-public-literature-portfolio/0.1",
        )
        self.assertEqual(portfolio["lanes"][0]["specialty"], "glioma")
        self.assertEqual(portfolio["total_returned_count"], 2)
        self.assertEqual(client.calls[-1][0], "neurosurgery_public_literature_portfolio")
        self.assertEqual(client.calls[-1][1]["query"]["max_hits_per_lane"], 2)
        self.assertEqual(client.calls[-1][1]["query"]["specialties"], ["glioma", "chiari_malformation"])
        with self.assertRaises(ArgumentError):
            agent.public_literature_portfolio({}, max_hits_per_lane=0)
        with self.assertRaises(ArgumentError):
            agent.public_literature_portfolio({}, from_date="2026-02-30")
        with self.assertRaises(ArgumentError):
            agent.public_literature_portfolio({}, from_date="2026-01-02", to_date="2026-01-01")
        literature_context = agent.public_literature_reasoning_context(
            {"schema_version": "bioprism-neurosurgery-public-literature/0.1"},
            packet={"query": {"specialty": "chiari_malformation", "limit": 2}},
            max_chars=6000,
            include_abstracts=True,
        )
        self.assertEqual(
            literature_context["schema_version"],
            "bioprism-neurosurgery-public-literature-reasoning-context/0.1",
        )
        self.assertEqual(client.calls[-1][0], "neurosurgery_public_literature_reasoning_context")
        self.assertEqual(client.calls[-1][1]["query"]["packet"]["query"]["limit"], 2)
        self.assertEqual(client.calls[-1][1]["query"]["max_chars"], 6000)
        self.assertTrue(client.calls[-1][1]["query"]["include_abstracts"])
        literature_draft = agent.public_literature_draft_audit(
            {"schema_version": "bioprism-neurosurgery-public-literature/0.1"},
            [{
                "claim_id": "chiari-citation",
                "kind": "source_observation",
                "scope": "citation_metadata",
                "text": "The packet contains a source-linked PMID.",
                "citations": [{"record_kind": "literature_article", "record_id": "1"}],
            }],
            query={"specialty": "chiari_malformation", "limit": 1},
        )
        self.assertEqual(
            literature_draft["schema_version"],
            "bioprism-neurosurgery-public-literature-draft-audit/0.1",
        )
        self.assertEqual(client.calls[-1][0], "neurosurgery_public_literature_draft_audit")
        self.assertEqual(client.calls[-1][1]["query"]["query"]["limit"], 1)
        matrix = agent.public_literature_matrix(
            {"schema_version": "bioprism-neurosurgery-public-literature/0.1"},
            specialties=["glioma", "chiari_malformation"],
            query={"text": "glioma", "limit": 2},
        )
        self.assertEqual(
            matrix["schema_version"],
            "bioprism-neurosurgery-public-literature-matrix/0.1",
        )
        self.assertEqual(client.calls[-1][0], "neurosurgery_public_literature_matrix")
        self.assertEqual(
            client.calls[-1][1]["query"]["specialties"],
            ["glioma", "chiari_malformation"],
        )
        self.assertEqual(client.calls[-1][1]["query"]["query"]["limit"], 2)
        draft = agent.real_data_draft_audit(
            {"schema_version": "bioprism-neurosurgery-real/0.1"},
            [{
                "claim_id": "trial-metadata",
                "kind": "source_observation",
                "scope": "public_record_metadata",
                "text": "The packet contains a public registry record.",
                "citations": [{"record_kind": "clinical_trial", "record_id": "NCT00005955"}],
            }],
            query={"query": {"text": "glioblastoma", "limit": 4}},
        )
        self.assertEqual(
            draft["schema_version"],
            "bioprism-neurosurgery-real-data-draft-audit/0.1",
        )
        self.assertEqual(draft["status"], "grounded_for_human_review")
        self.assertEqual(client.calls[-1][0], "neurosurgery_real_data_draft_audit")
        self.assertEqual(len(client.calls[-1][1]["claims"]), 1)
        self.assertEqual(client.calls[-1][1]["query"]["query"]["limit"], 4)
        queried = agent.query_real_data(
            {"schema_version": "bioprism-neurosurgery-real/0.1"},
            text="GBM",
            publication_type="systematic review",
            mesh_term="glioblastoma",
            publication_date_from="2019-01-01",
            publication_date_to="2019-12-31",
            record_kind="literature_article",
            source_id="pubmed_glioblastoma",
            related_record_id="gbm_tcga_pub2013",
        )
        self.assertEqual(queried["returned_matches"], 1)
        self.assertEqual(
            client.calls[-1][1]["query"]["record_kind"],
            "literature_article",
        )
        self.assertEqual(
            client.calls[-1][1]["query"]["related_record_id"],
            "gbm_tcga_pub2013",
        )
        self.assertEqual(
            client.calls[-1][1]["query"]["publication_type"],
            "systematic review",
        )
        self.assertEqual(
            client.calls[-1][1]["query"]["mesh_term"],
            "glioblastoma",
        )
        self.assertEqual(
            client.calls[-1][1]["query"]["publication_date_from"],
            "2019-01-01",
        )
        self.assertEqual(
            client.calls[-1][1]["query"]["publication_date_to"],
            "2019-12-31",
        )
        trial_queried = agent.query_real_data(
            {"schema_version": "bioprism-neurosurgery-real/0.1"},
            text="glioblastoma",
            trial_phase="PHASE2",
            trial_study_type="INTERVENTIONAL",
            trial_updated_from="2023-01-01",
            trial_updated_to="2024-12-31",
            record_kind="clinical_trial",
        )
        self.assertEqual(trial_queried["returned_matches"], 1)
        self.assertEqual(client.calls[-1][1]["query"]["trial_phase"], "PHASE2")
        self.assertEqual(client.calls[-1][1]["query"]["trial_study_type"], "INTERVENTIONAL")
        self.assertEqual(client.calls[-1][1]["query"]["trial_updated_from"], "2023-01-01")
        self.assertEqual(client.calls[-1][1]["query"]["trial_updated_to"], "2024-12-31")
        with self.assertRaises(ArgumentError):
            agent.query_real_data(
                {"schema_version": "bioprism-neurosurgery-real/0.1"},
                publication_date_from="2020-01-01",
                publication_date_to="2019-12-31",
            )
        with self.assertRaises(ArgumentError):
            agent.query_real_data(
                {"schema_version": "bioprism-neurosurgery-real/0.1"},
                trial_updated_from="2024-01-01",
                trial_updated_to="2023-12-31",
            )
        agent.query_real_data(
            {"schema_version": "bioprism-neurosurgery-real/0.1"},
            genomic_data_type="Annotated Somatic Mutation",
            limit=4,
        )
        self.assertEqual(
            client.calls[-1][1]["query"]["genomic_data_type"],
            "Annotated Somatic Mutation",
        )
        landscape = agent.real_data_trial_landscape(
            {"schema_version": "bioprism-neurosurgery-real/0.1"},
            query={
                "query": {
                    "trial_phase": "phase2",
                    "trial_updated_from": "2023-01-01",
                    "trial_updated_to": "2024-12-31",
                }
            },
            max_interventions=16,
        )
        self.assertEqual(landscape["returned_trial_count"], 2)
        self.assertEqual(client.calls[-1][0], "neurosurgery_real_data_trial_landscape")
        self.assertEqual(
            client.calls[-1][1]["query"]["query"]["trial_phase"],
            "phase2",
        )
        self.assertEqual(client.calls[-1][1]["query"]["max_interventions"], 16)
        cohorts = agent.real_data_cohort_landscape(
            {"schema_version": "bioprism-neurosurgery-real/0.1"},
            query={"query": {"genomic_data_type": "Aligned Reads", "limit": 8}},
            max_projects=4,
        )
        self.assertEqual(cohorts["returned_project_count"], 2)
        self.assertEqual(client.calls[-1][0], "neurosurgery_real_data_cohort_landscape")
        self.assertEqual(
            client.calls[-1][1]["query"]["query"]["genomic_data_type"],
            "Aligned Reads",
        )
        self.assertEqual(client.calls[-1][1]["query"]["max_projects"], 4)
        with self.assertRaises(ArgumentError):
            agent.real_data_cohort_landscape(
                {"schema_version": "bioprism-neurosurgery-real/0.1"},
                query={"query": {"record_kind": "clinical_trial"}},
            )
        molecular = agent.real_data_molecular_coverage(
            {"schema_version": "bioprism-neurosurgery-real/0.1"},
            query={
                "query": {
                    "molecular_alteration_type": "mutation_extended",
                    "molecular_datatype": "maf",
                }
            },
            max_studies=8,
        )
        self.assertEqual(molecular["returned_profile_count"], 6)
        self.assertEqual(client.calls[-1][0], "neurosurgery_real_data_molecular_coverage")
        self.assertEqual(
            client.calls[-1][1]["query"]["query"]["molecular_datatype"],
            "maf",
        )
        self.assertEqual(client.calls[-1][1]["query"]["max_studies"], 8)
        with self.assertRaises(ArgumentError):
            agent.real_data_molecular_coverage(
                {"schema_version": "bioprism-neurosurgery-real/0.1"},
                query={"query": {"record_kind": "clinical_trial"}},
            )
        with self.assertRaises(ArgumentError):
            agent.real_data_molecular_coverage(
                {"schema_version": "bioprism-neurosurgery-real/0.1"},
                query={"query": {"genomic_data_type": "Aligned Reads"}},
            )
        with self.assertRaises(ArgumentError):
            agent.real_data_trial_landscape(
                {"schema_version": "bioprism-neurosurgery-real/0.1"},
                query={"query": {"record_kind": "literature_article"}},
            )
        with self.assertRaises(ArgumentError):
            agent.real_data_trial_landscape(
                {"schema_version": "bioprism-neurosurgery-real/0.1"},
                query={
                    "query": {
                        "trial_updated_from": "2025-01-01",
                        "trial_updated_to": "2024-01-01",
                    }
                },
            )
        queried_literature = agent.query_public_literature(
            {"schema_version": "bioprism-neurosurgery-public-literature/0.1"},
            specialty="chiari_malformation",
            text="chiari",
            publication_type="review",
            mesh_term="malformation",
            from_date="2020-01-01",
            to_date="2025-12-31",
            limit=3,
        )
        self.assertEqual(queried_literature["returned_matches"], 1)
        self.assertEqual(
            client.calls[-1][1]["query"]["specialty"],
            "chiari_malformation",
        )
        self.assertEqual(client.calls[-1][1]["query"]["publication_type"], "review")
        self.assertEqual(client.calls[-1][1]["query"]["mesh_term"], "malformation")
        self.assertEqual(client.calls[-1][1]["query"]["from_date"], "2020-01-01")
        self.assertEqual(client.calls[-1][1]["query"]["to_date"], "2025-12-31")
        request = {"specialty": "glioma", "request_use": "research_synthesis"}
        self.assertEqual(agent.plan(request)["status"], "ready_for_human_review")
        self.assertEqual(
            agent.plan(
                {"specialty": "chiari_malformation", "request_use": "research_synthesis"},
                public_literature={"schema_version": "bioprism-neurosurgery-public-literature/0.1"},
            )["status"],
            "ready_for_human_review",
        )
        self.assertEqual(
            agent.plan_with_public_literature(
                {"specialty": "chiari_malformation", "request_use": "research_synthesis"},
                {"schema_version": "bioprism-neurosurgery-public-literature/0.1"},
            )["status"],
            "ready_for_human_review",
        )
        checkpoints = list(agent.iter_session(request))
        self.assertEqual(len(checkpoints), 3)
        self.assertEqual(checkpoints[-1]["status"], "awaiting_human_review")
        result = agent.run_session(request)
        self.assertEqual(result["status"], "ready_for_human_review")
        self.assertTrue(any(name == "neurosurgery_session" for name, _ in client.calls))
        # The terminal hold is reached on the second route step; an exact bound must succeed.
        exact_bound = LocalNeurosurgicalAgent(FakeClient()).run_session(request, max_steps=2)
        self.assertEqual(exact_bound["status"], "ready_for_human_review")
        one_call = LocalNeurosurgicalAgent(FakeClient()).run_session_to_review(request, max_steps=2)
        self.assertEqual(one_call["steps_executed"], 2)
        self.assertEqual(one_call["session"]["status"], "awaiting_human_review")

    def test_facade_bounds_inputs_before_transport(self) -> None:
        agent = LocalNeurosurgicalAgent(FakeClient())
        with self.assertRaises(ValueError):
            agent.plan([])  # type: ignore[arg-type]
        with self.assertRaises(ValueError):
            agent.run_session({}, max_steps=0)
        with self.assertRaises(ValueError):
            agent.query_public_literature({}, from_date="2025-01-01", to_date="2024-01-01")
        with self.assertRaises(ValueError):
            agent.intake_portfolio("Review glioma evidence", include_all_specialties="yes")  # type: ignore[arg-type]

    def test_research_mission_is_provenance_first_and_requires_real_glioma_data(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        with self.assertRaises(ValueError):
            agent.run_research_mission({"specialty": "glioma"})
        bundle = {"schema_version": "bioprism-neurosurgery-real/0.1", "synthetic_data": False}
        mission = agent.run_research_mission(
            {"specialty": "glioma", "request_use": "research_synthesis"},
            real_glioma_data=bundle,
            query={"text": "GBM", "limit": 4},
            freshness={"as_of": "2027-08-31T00:00:00Z", "max_age_days": 30},
            max_steps=2,
        )
        self.assertEqual(mission["schema"], "bioprism-neurosurgical-research-mission/0.1")
        self.assertEqual(mission["provider"], "none")
        self.assertFalse(mission["network"])
        self.assertTrue(mission["human_review_required"])
        self.assertEqual(mission["real_data_query"]["returned_matches"], 1)
        self.assertEqual(mission["real_data_review_queue"]["provider"], "none")
        self.assertFalse(mission["real_data_evidence_packet"]["network"])
        self.assertEqual(
            client.calls[-1][1]["freshness"]["max_age_days"], 30
        )
        self.assertEqual(mission["real_data_evidence_graph"]["total_node_count"], 88)
        self.assertEqual(mission["real_data_evidence_graph"]["provider"], "none")
        self.assertFalse(mission["real_data_reasoning_context"]["synthetic_data"])
        self.assertIn("AURORA REAL-GLIOMA", mission["real_data_reasoning_context"]["context_text"])
        self.assertEqual(mission["research_plan"]["schema_version"], "bioprism-neurosurgery-research-plan/0.1")
        self.assertEqual(mission["research_plan"]["provider"], "none")
        self.assertEqual(mission["research_brief"]["schema_version"], "bioprism-neurosurgery-research-brief/0.1")
        self.assertEqual(mission["run"]["steps_executed"], 2)
        with self.assertRaises(ValueError):
            agent.run_research_mission(
                {"specialty": "glioma", "request_use": "research_synthesis"},
                real_glioma_data=bundle,
                case_asset_manifest_query={"requested_kinds": ["imaging_series"]},
                max_steps=2,
            )
        attached_manifest = {
            "schema_version": "bioprism-neurosurgery-case-asset-manifest/0.1",
            "specialty": "glioma",
            "synthetic_data": False,
            "direct_identifier_fields": [],
            "assets": [],
        }
        attached = agent.run_research_mission(
            {"specialty": "glioma", "request_use": "research_synthesis"},
            real_glioma_data=bundle,
            case_asset_manifest=attached_manifest,
            case_asset_manifest_query={"requested_kinds": ["imaging_series"], "max_review_items": 16},
            max_steps=2,
        )
        self.assertEqual(attached["provider"], "none")
        self.assertEqual(client.calls[-1][1]["case_asset_manifest"], attached_manifest)
        self.assertEqual(
            client.calls[-1][1]["case_asset_manifest_query"]["requested_kinds"], ["imaging_series"]
        )
        dicom_import = {
            "schema_version": "bioprism-neurosurgery-case-dicom-import/0.1",
            "specialty": "glioma",
            "deidentified": True,
            "synthetic_data": False,
            "source_id": "dicom-export",
            "datasets": [],
        }
        agent.run_research_mission(
            {"specialty": "glioma", "request_use": "research_synthesis"},
            real_glioma_data=bundle,
            case_dicom_import=dicom_import,
            max_steps=2,
        )
        self.assertEqual(client.calls[-1][1]["case_dicom_import"], dicom_import)
        fhir_import = {
            "schema_version": "bioprism-neurosurgery-case-fhir-import/0.1",
            "specialty": "glioma",
            "deidentified": True,
            "synthetic_data": False,
            "source_id": "fhir-export",
            "bundle": {"resourceType": "Bundle", "entry": []},
        }
        agent.run_research_mission(
            {"specialty": "glioma", "request_use": "research_synthesis"},
            real_glioma_data=bundle,
            case_fhir_import=fhir_import,
            max_steps=2,
        )
        self.assertEqual(client.calls[-1][1]["case_fhir_import"], fhir_import)
        agent.run_research_mission(
            {"specialty": "glioma", "request_use": "research_synthesis"},
            real_glioma_data=bundle,
            case_dicom_import=dicom_import,
            case_fhir_import=fhir_import,
            max_steps=2,
        )
        self.assertEqual(client.calls[-1][1]["case_dicom_import"], dicom_import)
        self.assertEqual(client.calls[-1][1]["case_fhir_import"], fhir_import)
        disposition_ledger = {
            "schema_version": "bioprism-neurosurgery-case-asset-review-disposition/0.1",
            "report_digest": "d" * 64,
            "disposition_digest": "x" * 64,
            "candidate_item_count": 0,
            "returned_item_count": 0,
            "omitted_item_count": 0,
            "submitted_decision_count": 0,
            "accepted_decision_count": 0,
            "resolved_decision_count": 0,
            "unresolved_decision_count": 0,
            "undecided_returned_item_count": 0,
            "pending_item_count": 0,
            "decisions": [],
            "unresolved_sequences": [],
            "undecided_sequences": [],
            "provenance_bound": True,
            "synthetic_data": False,
            "human_review_required": True,
            "provider": "none",
            "network": False,
            "effect": "read_only",
            "limitations": [],
        }
        agent.run_research_mission(
            {"specialty": "glioma", "request_use": "research_synthesis"},
            real_glioma_data=bundle,
            case_asset_manifest=attached_manifest,
            case_asset_review_disposition=disposition_ledger,
            max_steps=2,
        )
        self.assertEqual(
            client.calls[-1][1]["case_asset_review_disposition"], disposition_ledger
        )
        dual_mission = agent.run_research_mission(
            {"specialty": "glioma", "request_use": "research_synthesis"},
            real_glioma_data={"schema_version": "bioprism-neurosurgery-real/0.1", "synthetic_data": False},
            public_literature={"schema_version": "bioprism-neurosurgery-public-literature/0.1", "synthetic_data": False},
            query={"text": "GBM", "limit": 1},
            public_literature_query={"specialty": "glioma", "text": "glioma", "limit": 1},
            max_steps=2,
        )
        self.assertEqual(dual_mission["literature_link_audit"]["schema_version"], "bioprism-neurosurgery-literature-link-audit/0.1")
        self.assertEqual(client.calls[-1][1]["public_literature_query"]["limit"], 1)
        agent.intake_mission(
            "Route this glioma case through the evidence workflow",
            specialty="glioma",
            real_glioma_data=bundle,
            case_request={
                "case_id": "case-deidentified-001",
                "specialty": "glioma",
                "request_use": "research_synthesis",
                "question": "transient case question",
                "observations": [],
            },
        )
        self.assertEqual(
            client.calls[-1][1]["case_request"]["case_id"], "case-deidentified-001"
        )

    def test_persisted_mission_replay_uses_the_existing_mission_tool(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        result = agent.validate_mission(
            {"specialty": "glioma", "request_use": "research_synthesis"},
            {"mission_id": "neurosurgical-mission-test"},
            real_glioma_data={"schema_version": "bioprism-neurosurgery-real/0.1", "synthetic_data": False},
            case_fhir_import={"schema_version": "bioprism-neurosurgery-case-fhir-import/0.1"},
        )
        self.assertTrue(result["valid"])
        self.assertEqual(client.calls[-1][0], "neurosurgery_mission")
        self.assertEqual(client.calls[-1][1]["operation"], "validate")
        self.assertIn("mission", client.calls[-1][1])
        self.assertEqual(
            client.calls[-1][1]["case_fhir_import"]["schema_version"],
            "bioprism-neurosurgery-case-fhir-import/0.1",
        )

    def test_public_literature_session_and_mission_bind_the_bundle(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        request = {"specialty": "encephalocele", "request_use": "research_synthesis"}
        bundle = {"schema_version": "bioprism-neurosurgery-public-literature/0.1", "synthetic_data": False}
        with self.assertRaises(ValueError):
            agent.run_research_mission(request, max_steps=2)
        started = agent.start_session(request, public_literature=bundle)
        self.assertEqual(started["status"], "planned")
        advanced = agent.advance_session(request, started, public_literature=bundle)
        advanced = agent.advance_session(request, advanced, public_literature=bundle)
        self.assertEqual(advanced["status"], "awaiting_human_review")
        finished = agent.finish_session(request, advanced, public_literature=bundle)
        self.assertEqual(finished["status"], "ready_for_human_review")
        run = agent.run_session_to_review(request, public_literature=bundle, max_steps=2)
        self.assertEqual(run["session"]["status"], "awaiting_human_review")
        mission = agent.run_research_mission(
            request,
            public_literature=bundle,
            query={"specialty": "encephalocele", "text": "encephalocele", "limit": 2},
            portfolio_query={
                "specialties": ["encephalocele", "glioma"],
                "max_hits_per_lane": 1,
                "max_review_items_per_lane": 1,
                "max_issues_per_lane": 1,
            },
            freshness={"as_of": "2027-08-31T00:00:00Z", "max_age_days": 30},
            max_steps=2,
        )
        self.assertEqual(mission["provider"], "none")
        self.assertEqual(
            mission["public_literature_integrity_audit"]["schema_version"],
            "bioprism-neurosurgery-public-literature-integrity-audit/0.1",
        )
        self.assertEqual(
            mission["public_literature_review_queue"]["schema_version"],
            "bioprism-neurosurgery-public-literature-review-queue/0.1",
        )
        self.assertEqual(
            mission["public_literature_workbench"]["schema_version"],
            "bioprism-neurosurgery-public-literature-workbench/0.1",
        )
        self.assertEqual(
            mission["public_literature_portfolio"]["schema_version"],
            "bioprism-neurosurgery-public-literature-portfolio/0.1",
        )
        self.assertEqual(mission["public_literature_portfolio"]["specialty_count"], 2)
        self.assertEqual(mission["public_literature_portfolio"]["total_match_count"], 48)
        self.assertEqual(client.calls[-1][1]["public_literature"]["schema_version"], bundle["schema_version"])
        self.assertEqual(client.calls[-1][1]["freshness"]["as_of"], "2027-08-31T00:00:00Z")
        handoff = agent.plan_research(request, public_literature=bundle, max_tasks=4, max_references_per_task=2)
        self.assertEqual(handoff["schema_version"], "bioprism-neurosurgery-research-plan/0.1")
        self.assertEqual(client.calls[-1][0], "neurosurgery_research_plan")
        self.assertEqual(client.calls[-1][1]["public_literature"]["schema_version"], bundle["schema_version"])

    def test_grounded_real_data_research_composes_local_model_and_draft_audit(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()

        def local_handler(request: Any) -> Mapping[str, Any]:
            self.assertEqual(request.model, "llama3.1")
            return {
                "answer": "The real-data packet exposes a TCGA-GBM molecular project.",
                "unknowns": ["The packet does not establish a patient-specific conclusion."],
                "claims": [
                    {
                        "claim_id": "gbm-project",
                        "kind": "population_summary",
                        "scope": "public_record_metadata",
                        "text": "The packet contains the TCGA-GBM project.",
                        "citations": [
                            {
                                "record_kind": "genomic_project",
                                "record_id": "TCGA-GBM",
                            }
                        ],
                    }
                ],
            }

        runtime.register_in_memory_provider(
            "ollama",
            local_handler,
            protocol="openai_chat_completions",
            structured_output_mode="json_object",
        )
        bundle = {
            "schema_version": "bioprism-neurosurgery-real/0.1",
            "synthetic_data": False,
        }
        grounded = agent.grounded_real_data_research(
            "What public molecular project is represented?",
            bundle,
            runtime,
            provider="ollama",
            model="llama3.1",
            approve_provider_call=True,
            include_abstracts=False,
            freshness={"as_of": "2026-08-31T00:00:00Z", "max_age_days": 365},
            real_data_query={
                "record_kind": "genomic_project",
                "genomic_data_type": "Annotated Somatic Mutation",
                "limit": 1,
            },
        )
        self.assertEqual(grounded["schema_version"], "bioprism-neurosurgery-grounded-research/0.1")
        self.assertEqual(grounded["status"], "grounded_for_human_review")
        self.assertEqual(grounded["transport"], "in_memory")
        self.assertTrue(grounded["human_review_required"])
        self.assertEqual(grounded["audit"]["status"], "grounded_for_human_review")
        self.assertEqual(len(grounded["claims"]), 1)
        self.assertEqual(
            [name for name, _ in client.calls[-2:]],
            ["neurosurgery_real_data_reasoning_context", "neurosurgery_real_data_draft_audit"],
        )
        self.assertEqual(client.calls[-2][1]["query"]["packet"]["freshness"]["max_age_days"], 365)
        self.assertEqual(
            client.calls[-2][1]["query"]["packet"]["query"],
            {
                "record_kind": "genomic_project",
                "genomic_data_type": "Annotated Somatic Mutation",
                "limit": 1,
                "text": "What public molecular project is represented?",
            },
        )
        with self.assertRaises(ArgumentError):
            agent.grounded_real_data_research(
                "What public molecular project is represented?",
                bundle,
                runtime,
                provider="ollama",
                model="llama3.1",
                approve_provider_call=True,
                real_data_query={"publication_date_from": "2026-02-30"},
            )
        with self.assertRaises(ArgumentError):
            agent.grounded_real_data_research(
                "What public molecular project is represented?",
                bundle,
                runtime,
                provider="ollama",
                model="llama3.1",
            )

    def test_grounded_real_data_tool_loop_executes_only_snapshot_search_and_closes_citations(self) -> None:
        class ToolSearchClient(FakeClient):
            def call_tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> ToolResult:
                if name == "neurosurgery_real_data_query":
                    args = dict(arguments or {})
                    self.calls.append((name, args))
                    payload = {
                        "schema_version": "bioprism-neurosurgery-real/0.1",
                        "query": args["query"],
                        "total_matches": 1,
                        "returned_matches": 1,
                        "truncated": False,
                        "hits": [{
                            "record_kind": "clinical_trial",
                            "record_id": "TOOL-TRIAL",
                            "title": "Tool-discovered trial",
                            "source_id": "clinicaltrials_glioma",
                            "source_uri": "https://clinicaltrials.gov/study/TOOL-TRIAL",
                            "record_uri": "https://clinicaltrials.gov/study/TOOL-TRIAL",
                        }],
                    }
                    return ToolResult(tool=name, envelope={"content": [{"type": "text", "text": json.dumps(payload)}]})
                return super().call_tool(name, arguments)

        client = ToolSearchClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        turns = 0

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal turns
            turns += 1
            if turns == 1:
                return {
                    "tool_calls": [{
                        "id": "search-1",
                        "name": "neurosurgery_real_data_search",
                        "arguments": {"text": "trial", "limit": 1},
                    }]
                }
            return {
                "answer": "The tool returned one source-linked trial row.",
                "unknowns": [],
                "claims": [{
                    "claim_id": "tool-trial",
                    "kind": "source_observation",
                    "scope": "public_record_metadata",
                    "text": "A bounded query returned a clinical-trial metadata row.",
                    "citations": [{"record_kind": "clinical_trial", "record_id": "TOOL-TRIAL"}],
                }],
            }

        runtime.register_in_memory_provider(
            "ollama", local_handler, protocol="openai_chat_completions", structured_output_mode="json_object"
        )
        result = agent.grounded_real_data_research(
            "Find glioma trial metadata.",
            {"schema_version": "bioprism-neurosurgery-real/0.1", "synthetic_data": False},
            runtime,
            "ollama",
            "llama3.1",
            approve_provider_call=True,
            tool_loop=True,
        )
        self.assertEqual(result["tool_loop"], {"status": "completed", "turns": 2, "tool_calls": 1})
        self.assertEqual(result["tool_trace"][0]["tool"], "neurosurgery_real_data_search")
        self.assertNotIn("text", result["tool_trace"][0]["query"])
        self.assertEqual(result["tool_trace"][0]["query"]["text_bytes"], len("trial".encode("utf-8")))
        self.assertEqual(result["audit"]["status"], "grounded_for_human_review")
        self.assertEqual(
            [name for name, _ in client.calls],
            [
                "neurosurgery_real_data_reasoning_context",
                "neurosurgery_real_data_query",
                "neurosurgery_real_data_draft_audit",
            ],
        )

    def test_grounded_real_data_tool_loop_supports_structured_facets_without_widening_scope(self) -> None:
        class ToolSearchClient(FakeClient):
            def call_tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> ToolResult:
                if name == "neurosurgery_real_data_query":
                    args = dict(arguments or {})
                    self.calls.append((name, args))
                    return ToolResult(tool=name, envelope={"content": [{"type": "text", "text": json.dumps({
                        "schema_version": "bioprism-neurosurgery-real/0.1",
                        "query": args["query"], "total_matches": 1, "returned_matches": 1,
                        "truncated": False,
                        "hits": [{"record_kind": "clinical_trial", "record_id": "FACET-TRIAL",
                                  "title": "Interventional trial", "source_id": "clinicaltrials_glioma",
                                  "source_uri": "https://clinicaltrials.gov/study/FACET-TRIAL",
                                  "status": "RECRUITING", "phases": ["PHASE2"],
                                  "study_type": "Interventional", "enrollment_count": 42,
                                  "intervention_names": ["metadata-only intervention label"],
                                  "abstract_excerpt": "A bounded abstract excerpt.",
                                  "related_records": [
                                      {"record_kind": "portal_study", "record_id": "GBM-STUDY", "relation": "describes_study"},
                                      {"record_kind": "patient_case", "record_id": "SHOULD-DROP", "relation": "has_profile"},
                                      {"record_kind": "portal_study", "record_id": "SHOULD-DROP", "relation": "unsupported"},
                                  ]}],
                    })}]})
                return super().call_tool(name, arguments)

        client = ToolSearchClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        turns = 0

        continuation_request: Any = None

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal turns
            nonlocal continuation_request
            turns += 1
            if turns == 1:
                tool_properties = request.tools[0].parameters["properties"]
                self.assertIn("trial_study_type", tool_properties)
                self.assertIn("record_kind", tool_properties)
                return {"tool_calls": [{"id": "facet-search", "name": "neurosurgery_real_data_search",
                                         "arguments": {"record_kind": "clinical_trial",
                                                       "trial_study_type": "Interventional", "limit": 128}}]}
            continuation_request = request
            return {"answer": "The structured search returned one trial row.", "unknowns": [], "claims": [{
                "claim_id": "facet-trial", "kind": "source_observation", "scope": "public_record_metadata",
                "text": "A bounded structured query returned a clinical-trial metadata row.",
                "citations": [{"record_kind": "clinical_trial", "record_id": "FACET-TRIAL"}],
            }]}

        runtime.register_in_memory_provider("ollama", local_handler, protocol="openai_chat_completions", structured_output_mode="json_object")
        result = agent.grounded_real_data_research(
            "Find recruiting trials.",
            {"schema_version": "bioprism-neurosurgery-real/0.1", "synthetic_data": False},
            runtime, "ollama", "llama3.1", approve_provider_call=True, tool_loop=True,
            real_data_query={"record_kind": "clinical_trial", "limit": 1},
        )
        query_call = next(args for name, args in client.calls if name == "neurosurgery_real_data_query")
        self.assertEqual(query_call["query"]["record_kind"], "clinical_trial")
        self.assertEqual(query_call["query"]["trial_study_type"], "Interventional")
        self.assertEqual(query_call["query"]["limit"], 1)
        tool_message = next(message for message in continuation_request.messages if message.get("role") == "tool")
        tool_payload = json.loads(tool_message["content"])
        self.assertEqual(tool_payload["hits"][0]["status"], "RECRUITING")
        self.assertEqual(tool_payload["hits"][0]["phases"], ["PHASE2"])
        self.assertEqual(tool_payload["hits"][0]["enrollment_count"], 42)
        self.assertEqual(tool_payload["hits"][0]["abstract_excerpt"], "A bounded abstract excerpt.")
        self.assertEqual(tool_payload["hits"][0]["related_records"], [{
            "record_kind": "portal_study", "record_id": "GBM-STUDY", "relation": "describes_study"
        }])
        self.assertEqual(result["tool_trace"][0]["query"]["trial_study_type"], "Interventional")
        self.assertNotIn("text", result["tool_trace"][0]["query"])

    def test_grounded_real_data_tool_loop_exposes_trial_landscape_view_with_citations(self) -> None:
        class LandscapeClient(FakeClient):
            def call_tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> ToolResult:
                if name == "neurosurgery_real_data_query":
                    args = dict(arguments or {})
                    self.calls.append((name, args))
                    return ToolResult(tool=name, envelope={"content": [{"type": "text", "text": json.dumps({
                        "schema_version": "bioprism-neurosurgery-real/0.1",
                        "query": args["query"], "total_matches": 1, "returned_matches": 1, "truncated": False,
                        "hits": [{"record_kind": "clinical_trial", "record_id": "VIEW-TRIAL",
                                  "title": "Recruiting glioma trial", "source_id": "clinicaltrials_glioma",
                                  "source_uri": "https://clinicaltrials.gov/study/VIEW-TRIAL",
                                  "status": "RECRUITING", "phases": ["PHASE2"],
                                  "study_type": "Interventional", "enrollment_count": 37,
                                  "intervention_names": ["metadata-only label"]}],
                    })} ]})
                return super().call_tool(name, arguments)

        client = LandscapeClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        turns = 0
        continuation_request: Any = None

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal turns, continuation_request
            turns += 1
            if turns == 1:
                names = [tool.name for tool in request.tools]
                self.assertIn(NEUROSURGERY_GROUNDED_REAL_TRIAL_LANDSCAPE_TOOL, names)
                self.assertIn(NEUROSURGERY_GROUNDED_REAL_MOLECULAR_COVERAGE_TOOL, names)
                return {"tool_calls": [{"id": "trial-view", "name": NEUROSURGERY_GROUNDED_REAL_TRIAL_LANDSCAPE_TOOL,
                                         "arguments": {"trial_study_type": "Interventional", "limit": 128,
                                                       "max_interventions": 4}}]}
            continuation_request = request
            return {"answer": "The bounded registry view found one trial row.", "unknowns": [], "claims": [{
                "claim_id": "view-trial", "kind": "source_observation", "scope": "public_record_metadata",
                "text": "The trial-landscape view returned one recruiting metadata row.",
                "citations": [{"record_kind": "clinical_trial", "record_id": "VIEW-TRIAL"}],
            }]}

        runtime.register_in_memory_provider("ollama", local_handler, protocol="openai_chat_completions", structured_output_mode="json_object")
        result = agent.grounded_real_data_research(
            "Reconnoiter recruiting glioma trials.",
            {"schema_version": "bioprism-neurosurgery-real/0.1", "synthetic_data": False},
            runtime, "ollama", "llama3.1", approve_provider_call=True, tool_loop=True,
            real_data_query={"record_kind": "clinical_trial", "limit": 1},
        )
        query_calls = [args for name, args in client.calls if name == "neurosurgery_real_data_query"]
        self.assertEqual(query_calls[0]["query"]["record_kind"], "clinical_trial")
        self.assertEqual(query_calls[0]["query"]["trial_study_type"], "Interventional")
        self.assertEqual(query_calls[0]["query"]["limit"], 1)
        tool_message = next(message for message in continuation_request.messages if message.get("role") == "tool")
        tool_payload = json.loads(tool_message["content"])
        self.assertEqual(tool_payload["view"], "trial_landscape")
        self.assertEqual(tool_payload["summary"]["total_matching_trials"], 2)
        self.assertFalse(tool_payload["summary"]["synthetic_data"])
        self.assertEqual(tool_payload["hits"][0]["record_id"], "VIEW-TRIAL")
        self.assertEqual(result["tool_trace"][0]["summary_digest"], "l" * 64)

    def test_grounded_real_data_tool_loop_exposes_molecular_coverage_view_without_patient_values(self) -> None:
        class CoverageClient(FakeClient):
            def call_tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> ToolResult:
                if name == "neurosurgery_real_data_query":
                    args = dict(arguments or {})
                    self.calls.append((name, args))
                    return ToolResult(tool=name, envelope={"content": [{"type": "text", "text": json.dumps({
                        "schema_version": "bioprism-neurosurgery-real/0.1",
                        "query": args["query"], "total_matches": 1, "returned_matches": 1, "truncated": False,
                        "hits": [{"record_kind": "portal_molecular_profile", "record_id": "VIEW-PROFILE",
                                  "title": "GBM mutation profile", "source_id": "cbioportal_gbm_catalog",
                                  "source_uri": "https://www.cbioportal.org/", "molecular_alteration_type": "MUTATION_EXTENDED",
                                  "datatype": "MAF", "molecular_description": "public assay metadata",
                                  "molecular_patient_level": True}],
                    })} ]})
                return super().call_tool(name, arguments)

        client = CoverageClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        turns = 0
        continuation_request: Any = None

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal turns, continuation_request
            turns += 1
            if turns == 1:
                return {"tool_calls": [{"id": "coverage-view", "name": NEUROSURGERY_GROUNDED_REAL_MOLECULAR_COVERAGE_TOOL,
                                         "arguments": {"molecular_datatype": "MAF", "limit": 128, "max_studies": 2}}]}
            continuation_request = request
            return {"answer": "The bounded molecular coverage view found one profile metadata row.", "unknowns": [], "claims": [{
                "claim_id": "view-profile", "kind": "source_observation", "scope": "public_record_metadata",
                "text": "The molecular-coverage view returned one public assay metadata row.",
                "citations": [{"record_kind": "portal_molecular_profile", "record_id": "VIEW-PROFILE"}],
            }]}

        runtime.register_in_memory_provider("ollama", local_handler, protocol="openai_chat_completions", structured_output_mode="json_object")
        result = agent.grounded_real_data_research(
            "Inventory MAF coverage for glioma.",
            {"schema_version": "bioprism-neurosurgery-real/0.1", "synthetic_data": False},
            runtime, "ollama", "llama3.1", approve_provider_call=True, tool_loop=True,
            real_data_query={"record_kind": "portal_molecular_profile", "limit": 1},
        )
        tool_message = next(message for message in continuation_request.messages if message.get("role") == "tool")
        tool_payload = json.loads(tool_message["content"])
        self.assertEqual(tool_payload["view"], "molecular_coverage")
        self.assertEqual(tool_payload["summary"]["total_matching_profile_count"], 6)
        self.assertNotIn("patient_values", tool_payload["summary"])
        self.assertNotIn("patient_values", tool_payload["hits"][0])
        self.assertEqual(result["tool_trace"][0]["summary_digest"], "m" * 64)

    def test_grounded_real_data_tool_loop_exposes_identifier_reconciliation_view(self) -> None:
        class ReconciliationClient(FakeClient):
            def call_tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> ToolResult:
                if name == "neurosurgery_real_data_reconciliation":
                    args = dict(arguments or {})
                    self.calls.append((name, args))
                    payload = {
                        "schema_version": "bioprism-neurosurgery-real-data-reconciliation/0.1",
                        "reconciliation_digest": "r" * 64,
                        "bundle_digest": "b" * 64,
                        "generated_at": "2026-08-30T00:00:00Z",
                        "query": args["query"],
                        "counts": {
                            "portal_study_count": 1,
                            "portal_study_with_pmid_count": 1,
                            "portal_study_without_pmid_count": 0,
                            "portal_pmid_missing_literature_count": 1,
                            "shared_portal_pmid_count": 0,
                            "literature_article_count": 1,
                            "literature_with_doi_count": 1,
                            "shared_literature_doi_count": 0,
                        },
                        "candidate_issue_count": 1,
                        "returned_issue_count": 1,
                        "omitted_issue_count": 0,
                        "truncated": False,
                        "issues": [{
                            "kind": "portal_pmid_missing_literature",
                            "identifier": "99999999",
                            "record_kind": "portal_study",
                            "record_id": "gbm_tcga_pub",
                            "source_id": "cbioportal_gbm_catalog",
                            "related_record_ids": [],
                            "detail": "The portal PMID is not present in the literature snapshot.",
                            "patient_values": ["must-drop"],
                        }],
                        "requires_review": True,
                        "provenance_bound": True,
                        "synthetic_data": False,
                        "human_review_required": True,
                        "provider": "none",
                        "network": False,
                        "effect": "read_only",
                        "limitations": ["metadata-only identifier crosswalk"],
                    }
                    return ToolResult(tool=name, envelope={"content": [{"type": "text", "text": json.dumps(payload)}]})
                return super().call_tool(name, arguments)

        client = ReconciliationClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        turns = 0
        continuation_request: Any = None

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal turns, continuation_request
            turns += 1
            if turns == 1:
                self.assertIn(NEUROSURGERY_GROUNDED_REAL_RECONCILIATION_TOOL, [tool.name for tool in request.tools])
                return {"tool_calls": [{"id": "reconcile-view", "name": NEUROSURGERY_GROUNDED_REAL_RECONCILIATION_TOOL,
                                         "arguments": {"max_issues": 8}}]}
            continuation_request = request
            return {"answer": "The bundle has one unresolved identifier crosswalk obligation.", "unknowns": [], "claims": [{
                "claim_id": "reconcile-study", "kind": "source_observation", "scope": "public_record_metadata",
                "text": "One portal PMID is not represented in the literature snapshot.",
                "citations": [{"record_kind": "portal_study", "record_id": "gbm_tcga_pub"}],
            }]}

        runtime.register_in_memory_provider("ollama", local_handler, protocol="openai_chat_completions", structured_output_mode="json_object")
        result = agent.grounded_real_data_research(
            "Audit glioma identifier crosswalks.",
            {"schema_version": "bioprism-neurosurgery-real/0.1", "synthetic_data": False},
            runtime, "ollama", "llama3.1", approve_provider_call=True, tool_loop=True,
            real_data_query={"limit": 2},
        )
        reconciliation_call = next(args for name, args in client.calls if name == "neurosurgery_real_data_reconciliation")
        self.assertEqual(reconciliation_call["query"], {"max_issues": 2})
        tool_message = next(message for message in continuation_request.messages if message.get("role") == "tool")
        tool_payload = json.loads(tool_message["content"])
        self.assertEqual(tool_payload["view"], "identifier_reconciliation")
        self.assertEqual(tool_payload["returned_issues"], 1)
        self.assertEqual(tool_payload["issues"][0]["record_id"], "gbm_tcga_pub")
        self.assertNotIn("patient_values", tool_payload["issues"][0])
        self.assertEqual(tool_payload["summary"]["reconciliation_digest"], "r" * 64)
        self.assertEqual(result["tool_trace"][0]["summary_digest"], "r" * 64)

    def test_grounded_real_data_tool_loop_exposes_deterministic_topic_brief(self) -> None:
        class BriefClient(FakeClient):
            def call_tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> ToolResult:
                if name == "neurosurgery_research_brief":
                    args = dict(arguments or {})
                    self.calls.append((name, args))
                    payload = {
                        "schema_version": "bioprism-neurosurgical-research-brief/0.1",
                        "brief_digest": "q" * 64,
                        "request_digest": "x" * 64,
                        "source": "real_glioma",
                        "specialty": "glioma",
                        "bundle_digest": "b" * 64,
                        "generated_at": "2026-08-31T00:00:00Z",
                        "query": args["query"],
                        "topic_count": 1,
                        "non_empty_topic_count": 1,
                        "total_match_count": 1,
                        "total_returned_count": 1,
                        "cross_topic_record_count": 0,
                        "source_query_truncated": False,
                        "topics": [{
                            "topic_id": "molecular_identity",
                            "label": "Integrated molecular identity",
                            "terms": ["idh", "mgmt"],
                            "matched_record_count": 1,
                            "returned_record_count": 1,
                            "truncated": False,
                            "source_ids": ["pubmed_glioma_molecular"],
                            "publication_type_counts": [{"label": "Review", "count": 1}],
                            "abstract_count": 0,
                            "records": [{
                                "source": "real_glioma",
                                "specialty": "glioma",
                                "record_kind": "literature_article",
                                "record_id": "12345678",
                                "title": "IDH and MGMT in diffuse glioma",
                                "source_id": "pubmed_glioma_molecular",
                                "source_uri": "https://pubmed.ncbi.nlm.nih.gov/",
                                "record_uri": "https://pubmed.ncbi.nlm.nih.gov/12345678/",
                                "publication_date": "2024-01-01",
                                "matched_terms": ["idh", "mgmt"],
                                "publication_types": ["Review"],
                                "mesh_terms": ["Glioma"],
                                "abstract_excerpt": "must not leak",
                            }],
                        }],
                        "unknowns": [{"code": "topic_unknown", "scope": "molecular_identity", "detail": "reviewer check"}],
                        "review_prompts": ["Confirm topic membership before synthesis."],
                        "provenance_bound": True,
                        "synthetic_data": False,
                        "human_review_required": True,
                        "provider": "none",
                        "network": False,
                        "effect": "read_only",
                        "limitations": ["lexical membership only"],
                    }
                    return ToolResult(tool=name, envelope={"content": [{"type": "text", "text": json.dumps(payload)}]})
                return super().call_tool(name, arguments)

        client = BriefClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        turns = 0
        continuation_request: Any = None

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal turns, continuation_request
            turns += 1
            if turns == 1:
                continuation_request = request
                self.assertIn(NEUROSURGERY_GROUNDED_REAL_RESEARCH_BRIEF_TOOL, [tool.name for tool in request.tools])
                return {"tool_calls": [{"id": "brief-view", "name": NEUROSURGERY_GROUNDED_REAL_RESEARCH_BRIEF_TOOL,
                                         "arguments": {"max_topics": 4, "max_records_per_topic": 2}}]}
            continuation_request = request
            return {"answer": "The deterministic topic lane found one exact source row.", "unknowns": [], "claims": [{
                "claim_id": "brief-record", "kind": "source_observation", "scope": "public_record_metadata",
                "text": "The topic lane returned one literature metadata row.",
                "citations": [{"record_kind": "literature_article", "record_id": "12345678"}],
            }]}

        runtime.register_in_memory_provider("ollama", local_handler, protocol="openai_chat_completions", structured_output_mode="json_object")
        result = agent.grounded_real_data_research(
            "Map glioma molecular identity topics.",
            {"schema_version": "bioprism-neurosurgery-real/0.1", "synthetic_data": False},
            runtime, "ollama", "llama3.1", approve_provider_call=True, tool_loop=True,
            real_data_query={"limit": 2},
        )
        brief_call = next(args for name, args in client.calls if name == "neurosurgery_research_brief")
        self.assertEqual(brief_call["query"]["max_topics"], 4)
        self.assertEqual(brief_call["query"]["include_abstracts"], False)
        tool_message = next(message for message in continuation_request.messages if message.get("role") == "tool")
        tool_payload = json.loads(tool_message["content"])
        self.assertEqual(tool_payload["view"], "topic_brief")
        self.assertEqual(tool_payload["returned_topics"], 1)
        self.assertEqual(tool_payload["topics"][0]["records"][0]["record_id"], "12345678")
        self.assertNotIn("abstract_excerpt", tool_payload["topics"][0]["records"][0])
        self.assertEqual(tool_payload["topics"][0]["records"][0]["publication_types"], ["Review"])
        self.assertEqual(tool_payload["topics"][0]["records"][0]["mesh_terms"], ["Glioma"])
        self.assertEqual(tool_payload["unknowns"][0]["code"], "topic_unknown")
        self.assertEqual(result["tool_trace"][0]["summary_digest"], "q" * 64)

    def test_grounded_real_data_tool_loop_exposes_cohort_landscape_with_exact_project_citations(self) -> None:
        class CohortClient(FakeClient):
            def call_tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> ToolResult:
                if name == "neurosurgery_real_data_cohort_landscape":
                    args = dict(arguments or {})
                    self.calls.append((name, args))
                    payload = {
                        "schema_version": "bioprism-neurosurgery-real-data-cohort-landscape/0.1",
                        "landscape_digest": "l" * 64,
                        "bundle_digest": "b" * 64,
                        "generated_at": "2026-08-31T00:00:00Z",
                        "query": args["query"],
                        "total_matching_projects": 2,
                        "returned_project_count": 1,
                        "omitted_project_count": 1,
                        "truncated": True,
                        "project_rows": [{
                            "project_id": "TCGA-GBM",
                            "source_id": "gdc_tcga_gbm",
                            "source_uri": "https://api.gdc.cancer.gov/projects/TCGA-GBM?format=json",
                            "name": "Glioblastoma Multiforme",
                            "primary_site": ["Brain"],
                            "disease_types": ["Gliomas"],
                            "case_count": 617,
                            "data_type_metadata_present": True,
                            "data_type_counts": [{"data_type": "Aligned Reads", "file_count": 3251}],
                            "total_file_count": 3251,
                        }],
                        "total_released_case_inventory": 617,
                        "data_type_coverage": [{"data_type": "Aligned Reads", "project_count": 1, "total_file_count": 3251}],
                        "shared_data_type_count": 0,
                        "shared_data_types": [],
                        "projects_with_data_type_metadata": 1,
                        "projects_without_data_type_metadata": 0,
                        "source_ids": ["gdc_tcga_gbm"],
                        "review_reasons": [{"code": "project_limit", "count": 1, "detail": "one project omitted by bound"}],
                        "provenance_bound": True,
                        "synthetic_data": False,
                        "human_review_required": True,
                        "provider": "none",
                        "network": False,
                        "effect": "read_only",
                        "limitations": ["aggregate metadata only"],
                    }
                    return ToolResult(tool=name, envelope={"content": [{"type": "text", "text": json.dumps(payload)}]})
                return super().call_tool(name, arguments)

        client = CohortClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        turns = 0
        continuation_request: Any = None

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal turns, continuation_request
            turns += 1
            if turns == 1:
                self.assertIn(NEUROSURGERY_GROUNDED_REAL_COHORT_LANDSCAPE_TOOL, [tool.name for tool in request.tools])
                return {"tool_calls": [{"id": "cohort-view", "name": NEUROSURGERY_GROUNDED_REAL_COHORT_LANDSCAPE_TOOL,
                                         "arguments": {"max_projects": 1}}]}
            continuation_request = request
            return {"answer": "The bounded cohort view exposes one source-linked TCGA project row.", "unknowns": [], "claims": [{
                "claim_id": "cohort-project", "kind": "source_observation", "scope": "public_record_metadata",
                "text": "The cohort landscape includes a TCGA-GBM project metadata row.",
                "citations": [{"record_kind": "genomic_project", "record_id": "TCGA-GBM"}],
            }]}

        runtime.register_in_memory_provider("ollama", local_handler, protocol="openai_chat_completions", structured_output_mode="json_object")
        result = agent.grounded_real_data_research(
            "Compare public glioma genomic projects.",
            {"schema_version": "bioprism-neurosurgery-real/0.1", "synthetic_data": False},
            runtime, "ollama", "llama3.1", approve_provider_call=True, tool_loop=True,
            real_data_query={"record_kind": "genomic_project", "genomic_data_type": "Aligned Reads", "limit": 2},
        )
        cohort_call = next(args for name, args in client.calls if name == "neurosurgery_real_data_cohort_landscape")
        self.assertEqual(cohort_call["query"]["max_projects"], 1)
        self.assertEqual(cohort_call["query"]["query"], {
            "record_kind": "genomic_project", "genomic_data_type": "Aligned Reads", "limit": 2,
        })
        tool_message = next(message for message in continuation_request.messages if message.get("role") == "tool")
        tool_payload = json.loads(tool_message["content"])
        self.assertEqual(tool_payload["view"], "cohort_landscape")
        self.assertEqual(tool_payload["project_rows"][0]["project_id"], "TCGA-GBM")
        self.assertEqual(tool_payload["project_rows"][0]["case_count"], 617)
        self.assertEqual(tool_payload["total_released_case_inventory"], 617)
        self.assertEqual(result["tool_trace"][0]["view"], "cohort_landscape")
        self.assertEqual(result["tool_trace"][0]["summary_digest"], "l" * 64)
        self.assertEqual(result["tool_trace"][0]["citations"], [{"record_kind": "genomic_project", "record_id": "TCGA-GBM"}])

    def test_grounded_real_data_views_reject_record_kind_drift(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        turns = 0

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal turns
            turns += 1
            if turns == 1:
                return {"tool_calls": [{"id": "bad-view", "name": NEUROSURGERY_GROUNDED_REAL_TRIAL_LANDSCAPE_TOOL,
                                         "arguments": {"record_kind": "genomic_project"}}]}
            return {"answer": "The caller-bound source remains available for review.", "unknowns": [], "claims": [{
                "claim_id": "view-drift", "kind": "source_observation", "scope": "public_record_metadata",
                "text": "The view rejected a record-kind drift.",
                "citations": [{"record_kind": "clinical_trial", "record_id": "NCT00000001"}],
            }]}

        runtime.register_in_memory_provider("ollama", local_handler, protocol="openai_chat_completions", structured_output_mode="json_object")
        result = agent.grounded_real_data_research(
            "Review trial metadata.",
            {"schema_version": "bioprism-neurosurgery-real/0.1", "synthetic_data": False},
            runtime, "ollama", "llama3.1", approve_provider_call=True, tool_loop=True,
            real_data_query={"record_kind": "clinical_trial", "limit": 1},
        )
        self.assertEqual(result["tool_trace"][0]["status"], "error")
        self.assertIn("fixed to record_kind=clinical_trial", result["tool_trace"][0]["error"])
        self.assertNotIn("neurosurgery_real_data_trial_landscape", [name for name, _ in client.calls])

    def test_grounded_real_data_tool_loop_exposes_review_queue_items_with_citations(self) -> None:
        class QueueClient(FakeClient):
            def call_tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> ToolResult:
                if name == "neurosurgery_real_data_review_queue":
                    args = dict(arguments or {})
                    self.calls.append((name, args))
                    payload = {
                        "schema_version": "bioprism-neurosurgery-real-data-review-queue/0.1",
                        "bundle_digest": "b" * 64, "queue_digest": "q" * 64,
                        "generated_at": "2026-08-30T00:00:00Z", "query": args["query"],
                        "source_count": 5, "record_count": 88, "candidate_item_count": 2,
                        "returned_item_count": 1, "omitted_item_count": 1, "truncated": True,
                        "items": [{
                            "task_id": "review-portal-1", "class": "provenance",
                            "kind": "missing_portal_publication_link", "status": "needs_human_review",
                            "source_id": "cbioportal_gbm_catalog", "source_kind": "study_portal",
                            "source_uri": "https://www.cbioportal.org/", "record_kind": "portal_study",
                            "record_id": "QUEUE-STUDY", "title": "Public glioma study",
                            "reason": "Verify whether a publication crosswalk exists.",
                            "reviewer_roles": ["neuro-oncology"], "patient_values": ["must-drop"],
                        }],
                        "provenance_bound": True, "synthetic_data": False,
                        "human_review_required": True, "provider": "none", "network": False,
                        "effect": "read_only", "limitations": ["metadata-only queue"],
                    }
                    return ToolResult(tool=name, envelope={"content": [{"type": "text", "text": json.dumps(payload)}]})
                return super().call_tool(name, arguments)

        client = QueueClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        turns = 0
        continuation_request: Any = None

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal turns, continuation_request
            turns += 1
            if turns == 1:
                self.assertIn(NEUROSURGERY_GROUNDED_REAL_REVIEW_QUEUE_TOOL, [tool.name for tool in request.tools])
                return {"tool_calls": [{"id": "queue-view", "name": NEUROSURGERY_GROUNDED_REAL_REVIEW_QUEUE_TOOL,
                                         "arguments": {"record_kind": "portal_study", "max_items": 128}}]}
            continuation_request = request
            return {"answer": "The snapshot has one unresolved public-study provenance task.", "unknowns": [], "claims": [{
                "claim_id": "queue-study", "kind": "source_observation", "scope": "public_record_metadata",
                "text": "A public-study publication crosswalk remains explicitly unresolved.",
                "citations": [{"record_kind": "portal_study", "record_id": "QUEUE-STUDY"}],
            }]}

        runtime.register_in_memory_provider("ollama", local_handler, protocol="openai_chat_completions", structured_output_mode="json_object")
        result = agent.grounded_real_data_research(
            "Find unresolved glioma provenance obligations.",
            {"schema_version": "bioprism-neurosurgery-real/0.1", "synthetic_data": False},
            runtime, "ollama", "llama3.1", approve_provider_call=True, tool_loop=True,
            real_data_query={"record_kind": "portal_study", "limit": 2},
        )
        queue_call = next(args for name, args in client.calls if name == "neurosurgery_real_data_review_queue")
        self.assertEqual(queue_call["query"], {"record_kind": "portal_study", "max_items": 2})
        tool_message = next(message for message in continuation_request.messages if message.get("role") == "tool")
        tool_payload = json.loads(tool_message["content"])
        self.assertEqual(tool_payload["view"], "review_queue")
        self.assertEqual(tool_payload["returned_items"], 1)
        self.assertNotIn("patient_values", tool_payload["items"][0])
        self.assertEqual(tool_payload["summary"]["queue_digest"], "q" * 64)
        self.assertEqual(result["tool_trace"][0]["summary_digest"], "q" * 64)
        self.assertEqual(result["audit"]["status"], "grounded_for_human_review")

    def test_grounded_real_data_tool_loop_exposes_evidence_graph_crosswalk(self) -> None:
        class GraphClient(FakeClient):
            def call_tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> ToolResult:
                if name == "neurosurgery_evidence_graph":
                    args = dict(arguments or {})
                    self.calls.append((name, args))
                    payload = {
                        "schema_version": "bioprism-neurosurgery-evidence-graph/0.1",
                        "bundle_digest": "b" * 64, "graph_digest": "g" * 64,
                        "specialty": "glioma", "query": args["query"],
                        "nodes": [
                            {"record_kind": "genomic_project", "record_id": "TCGA-GBM", "title": "TCGA glioblastoma project", "source_id": "gdc", "source_uri": "https://portal.gdc.cancer.gov/projects/TCGA-GBM"},
                            {"record_kind": "literature_article", "record_id": "GRAPH-PMID", "title": "Linked glioma citation", "source_id": "pubmed", "source_uri": "https://pubmed.ncbi.nlm.nih.gov/GRAPH-PMID/"},
                        ],
                        "edges": [{"from_record_kind": "genomic_project", "from_record_id": "TCGA-GBM", "to_record_kind": "literature_article", "to_record_id": "GRAPH-PMID", "relation": "published_as"}],
                        "total_node_count": 2, "total_edge_count": 1, "omitted_node_count": 0, "omitted_edge_count": 0,
                        "truncated": False, "root_count": 1, "connected_component_count": 1, "isolated_node_count": 0,
                        "source_count": 2, "bundle_relationship_count": 1,
                        "human_review_required": True, "provider": "none", "network": False, "effect": "read_only",
                        "limitations": ["explicit identifier crosswalk only"],
                    }
                    return ToolResult(tool=name, envelope={"content": [{"type": "text", "text": json.dumps(payload)}]})
                return super().call_tool(name, arguments)

        client = GraphClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        turns = 0

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal turns
            turns += 1
            if turns == 1:
                self.assertIn(NEUROSURGERY_GROUNDED_REAL_EVIDENCE_GRAPH_TOOL, [tool.name for tool in request.tools])
                return {"tool_calls": [{"id": "graph-view", "name": NEUROSURGERY_GROUNDED_REAL_EVIDENCE_GRAPH_TOOL,
                                         "arguments": {"root_record_id": "TCGA-GBM", "root_record_kind": "genomic_project", "max_nodes": 128, "max_edges": 256}}]}
            return {"answer": "The source crosswalk links a public project to a PMID.", "unknowns": [], "claims": [{
                "claim_id": "graph-link", "kind": "source_observation", "scope": "public_record_metadata",
                "text": "The bounded graph contains an explicit project-to-literature identifier edge.",
                "citations": [{"record_kind": "literature_article", "record_id": "GRAPH-PMID"}],
            }]}

        runtime.register_in_memory_provider("ollama", local_handler, protocol="openai_chat_completions", structured_output_mode="json_object")
        result = agent.grounded_real_data_research(
            "Inspect glioma source crosswalks.",
            {"schema_version": "bioprism-neurosurgery-real/0.1", "synthetic_data": False},
            runtime, "ollama", "llama3.1", approve_provider_call=True, tool_loop=True,
            real_data_query={"limit": 2},
        )
        graph_call = next(args for name, args in client.calls if name == "neurosurgery_evidence_graph")
        self.assertEqual(graph_call["query"], {"root_record_id": "TCGA-GBM", "root_record_kind": "genomic_project", "max_nodes": 2, "max_edges": 4})
        self.assertFalse(any(name == "neurosurgery_real_data_query" for name, _ in client.calls))
        self.assertEqual(result["tool_trace"][0]["summary_digest"], "g" * 64)
        self.assertEqual(result["tool_trace"][0]["summary"]["returned_node_count"], 2)
        self.assertEqual(result["claims"][0]["citations"][0]["record_id"], "GRAPH-PMID")

    def test_grounded_real_data_tool_loop_exposes_next_evidence_worklist(self) -> None:
        class AcquisitionClient(FakeClient):
            def call_tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> ToolResult:
                if name == "neurosurgery_evidence_acquisition":
                    args = dict(arguments or {})
                    self.calls.append((name, args))
                    payload = {
                        "schema_version": "bioprism-neurosurgery-evidence-acquisition/0.1",
                        "plan_digest": "a" * 64, "request_digest": "r" * 64, "specialty": "glioma",
                        "query": args.get("query", {}),
                        "audit": {},
                        "steps": [{
                            "sequence": 1, "step_id": "step-1", "source": "real_glioma_population",
                            "trigger": "missing_evidence_record", "observation_kind": None,
                            "query": {"source": "real_glioma_population", "query": {"record_kind": "clinical_trial", "limit": 2}},
                            "fallback_to_specialty_scan": False, "status": "candidates_found",
                            "total_matches": 2, "returned_matches": 2, "truncated": False,
                            "references": [{"source": "real_glioma_population", "source_id": "clinicaltrials_glioma", "record_id": "NCT00000001", "title": "Bounded trial metadata", "uri": "https://clinicaltrials.gov/study/NCT00000001"}],
                        }],
                        "candidate_step_count": 1, "omitted_step_count": 0, "truncated": False,
                        "source_query_count": 1, "source_candidate_count": 2, "required_sources": [],
                        "ready_for_local_replay": True, "human_review_required": True,
                        "provider": "none", "network": False, "effect": "read_only",
                        "limitations": ["local query worklist only"],
                    }
                    return ToolResult(tool=name, envelope={"content": [{"type": "text", "text": json.dumps(payload)}]})
                return super().call_tool(name, arguments)

        client = AcquisitionClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        turns = 0

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal turns
            turns += 1
            if turns == 1:
                self.assertIn(NEUROSURGERY_GROUNDED_REAL_EVIDENCE_ACQUISITION_TOOL, [tool.name for tool in request.tools])
                return {"tool_calls": [{"id": "acquisition-view", "name": NEUROSURGERY_GROUNDED_REAL_EVIDENCE_ACQUISITION_TOOL,
                                         "arguments": {"max_steps": 64, "max_references_per_step": 16}}]}
            return {"answer": "The bounded worker found a trial metadata query for reviewer replay.", "unknowns": [], "claims": [{
                "claim_id": "acquisition-plan", "kind": "limitation", "scope": "public_record_metadata",
                "text": "The next-evidence plan is a reviewer-owned local query, not a clinical finding.",
                "citations": [{"record_kind": "clinical_trial", "record_id": "NCT00000001"}],
            }]}

        runtime.register_in_memory_provider("ollama", local_handler, protocol="openai_chat_completions", structured_output_mode="json_object")
        result = agent.grounded_real_data_research(
            "Find the next bounded glioma evidence wave.",
            {"schema_version": "bioprism-neurosurgery-real/0.1", "synthetic_data": False},
            runtime, "ollama", "llama3.1", approve_provider_call=True, tool_loop=True,
            real_data_query={"limit": 2},
        )
        acquisition_call = next(args for name, args in client.calls if name == "neurosurgery_evidence_acquisition")
        self.assertEqual(acquisition_call["query"], {"max_steps": 2, "max_references_per_step": 16})
        self.assertEqual(result["tool_trace"][0]["summary_digest"], "a" * 64)
        self.assertEqual(result["tool_trace"][0]["summary"]["returned_step_count"], 1)
        self.assertEqual(result["audit"]["status"], "grounded_for_human_review")

    def test_grounded_real_data_tool_loop_exposes_specialty_evidence_map(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        turns = 0

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal turns
            turns += 1
            if turns == 1:
                self.assertTrue(any(tool.name == NEUROSURGERY_GROUNDED_SPECIALTY_EVIDENCE_MAP_TOOL for tool in request.tools))
                return {"tool_calls": [{"id": "specialty-map", "name": NEUROSURGERY_GROUNDED_SPECIALTY_EVIDENCE_MAP_TOOL, "arguments": {"max_dimensions": 32}}]}
            return {"answer": "The specialist map reports an explicit coverage hold for human review.", "unknowns": [], "claims": [{
                "claim_id": "map-hold", "kind": "limitation", "scope": "public_record_metadata",
                "text": "The specialist map is a coverage ledger and does not establish a patient finding.",
                "citations": [{"record_kind": "genomic_project", "record_id": "TCGA-GBM"}],
            }]}

        runtime.register_in_memory_provider("ollama", local_handler, protocol="openai_chat_completions", structured_output_mode="json_object")
        result = agent.grounded_real_data_research(
            "Map the specialist glioma evidence coverage.",
            {"schema_version": "bioprism-neurosurgery-real/0.1", "synthetic_data": False},
            runtime, "ollama", "llama3.1", approve_provider_call=True, tool_loop=True,
            real_data_query={"limit": 2},
        )
        map_call = next(args for name, args in client.calls if name == "neurosurgery_specialty_evidence_map")
        self.assertEqual(map_call["request"]["specialty"], "glioma")
        self.assertEqual(result["tool_trace"][0]["summary"]["specialty"], "glioma")
        self.assertEqual(result["tool_trace"][0]["map_digest"], "m" * 64)
        self.assertEqual(result["tool_trace"][0]["returned_dimensions"], 0)
        self.assertEqual(result["claims"][0]["citations"][0]["record_id"], "TCGA-GBM")

    def test_grounded_real_data_tool_loop_exposes_caller_clocked_freshness(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        turns = 0

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal turns
            turns += 1
            if turns == 1:
                self.assertTrue(any(tool.name == NEUROSURGERY_GROUNDED_REAL_FRESHNESS_TOOL for tool in request.tools))
                return {"tool_calls": [{"id": "freshness-view", "name": NEUROSURGERY_GROUNDED_REAL_FRESHNESS_TOOL, "arguments": {"max_sources": 8}}]}
            return {"answer": "The caller-clocked snapshot is stale and requires human review.", "unknowns": [], "claims": [{
                "claim_id": "freshness-hold", "kind": "limitation", "scope": "public_record_metadata",
                "text": "Source age is a metadata hold and does not establish a clinical finding.",
                "citations": [{"record_kind": "genomic_project", "record_id": "TCGA-GBM"}],
            }]}

        runtime.register_in_memory_provider("ollama", local_handler, protocol="openai_chat_completions", structured_output_mode="json_object")
        result = agent.grounded_real_data_research(
            "Check the freshness of the glioma snapshot.",
            {"schema_version": "bioprism-neurosurgery-real/0.1", "synthetic_data": False},
            runtime, "ollama", "llama3.1", approve_provider_call=True, tool_loop=True,
            freshness={"as_of": "2026-08-31T00:00:00Z", "max_age_days": 180},
            real_data_query={"limit": 1},
        )
        freshness_call = next(args for name, args in client.calls if name == "neurosurgery_real_data_freshness")
        self.assertEqual(freshness_call["query"], {"as_of": "2026-08-31T00:00:00Z", "max_age_days": 180})
        self.assertEqual(result["tool_trace"][0]["view"], "freshness")
        self.assertEqual(result["tool_trace"][0]["freshness_digest"], "f" * 64)
        self.assertEqual(result["tool_trace"][0]["freshness_status"], "stale")

    def test_grounded_real_data_freshness_requires_explicit_caller_clock(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        turns = 0

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal turns
            turns += 1
            if turns == 1:
                return {"tool_calls": [{"id": "freshness-no-clock", "name": NEUROSURGERY_GROUNDED_REAL_FRESHNESS_TOOL, "arguments": {}}]}
            return {"answer": "The freshness request was held for a caller clock.", "unknowns": [], "claims": [{
                "claim_id": "freshness-clock", "kind": "limitation", "scope": "public_record_metadata",
                "text": "Freshness cannot be evaluated without an explicit UTC caller clock.",
                "citations": [{"record_kind": "genomic_project", "record_id": "TCGA-GBM"}],
            }]}

        runtime.register_in_memory_provider("ollama", local_handler, protocol="openai_chat_completions", structured_output_mode="json_object")
        result = agent.grounded_real_data_research(
            "Check the freshness of the glioma snapshot.",
            {"schema_version": "bioprism-neurosurgery-real/0.1", "synthetic_data": False},
            runtime, "ollama", "llama3.1", approve_provider_call=True, tool_loop=True,
            real_data_query={"limit": 1},
        )
        self.assertEqual(result["tool_trace"][0]["status"], "error")
        self.assertIn("explicit caller freshness clock", result["tool_trace"][0]["error"])

    def test_grounded_real_data_tool_loop_exposes_snapshot_coverage(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        turns = 0

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal turns
            turns += 1
            if turns == 1:
                self.assertTrue(any(tool.name == NEUROSURGERY_GROUNDED_REAL_COVERAGE_TOOL for tool in request.tools))
                return {"tool_calls": [{"id": "coverage-view", "name": NEUROSURGERY_GROUNDED_REAL_COVERAGE_TOOL, "arguments": {"record_kind": "literature_article"}}]}
            return {"answer": "The snapshot coverage is a bounded metadata inventory with an explicit gap.", "unknowns": [], "claims": [{
                "claim_id": "coverage-hold", "kind": "limitation", "scope": "public_record_metadata",
                "text": "Coverage gaps remain reviewer-owned metadata obligations and do not establish a clinical finding.",
                "citations": [{"record_kind": "genomic_project", "record_id": "TCGA-GBM"}],
            }]}

        runtime.register_in_memory_provider("ollama", local_handler, protocol="openai_chat_completions", structured_output_mode="json_object")
        result = agent.grounded_real_data_research(
            "Audit source and temporal coverage of the glioma snapshot.",
            {"schema_version": "bioprism-neurosurgery-real/0.1", "synthetic_data": False},
            runtime, "ollama", "llama3.1", approve_provider_call=True, tool_loop=True,
            real_data_query={"limit": 2},
        )
        coverage_call = next(args for name, args in client.calls if name == "neurosurgery_real_data_coverage")
        self.assertEqual(coverage_call["query"], {"record_kind": "literature_article"})
        self.assertEqual(result["tool_trace"][0]["view"], "coverage")
        self.assertEqual(result["tool_trace"][0]["coverage_digest"], "c" * 64)
        self.assertEqual(result["tool_trace"][0]["returned_sources"], 1)
        self.assertEqual(result["tool_trace"][0]["returned_gaps"], 1)

    def test_grounded_real_data_specialty_evidence_map_rejects_lane_mismatch(self) -> None:
        class WrongLaneClient(FakeClient):
            def call_tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> ToolResult:
                if name == "neurosurgery_specialty_evidence_map":
                    payload = {
                        "schema_version": "bioprism-neurosurgery-specialty-evidence-map/0.1", "map_digest": "m" * 64,
                        "request_digest": "r" * 64, "specialty": "chiari_malformation", "dimensions": [],
                        "required_dimension_count": 0, "complete_dimension_count": 0, "partial_dimension_count": 0,
                        "not_collected_dimension_count": 0, "uninterpretable_dimension_count": 0, "conflicting_dimension_count": 0,
                        "observed_observation_count": 0, "evidence_record_count": 0, "verified_evidence_record_count": 0,
                        "missing_provenance_count": 0, "timestamped_observation_count": 0, "reviewer_questions": [],
                        "state": "not_collected", "provenance_bound": True, "synthetic_data": False,
                        "human_review_required": True, "provider": "none", "network": False, "effect": "read_only", "limitations": [],
                    }
                    return ToolResult(tool=name, envelope={"content": [{"type": "text", "text": json.dumps(payload)}]})
                return super().call_tool(name, arguments)

        client = WrongLaneClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        turns = 0

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal turns
            turns += 1
            if turns == 1:
                return {"tool_calls": [{"id": "wrong-map", "name": NEUROSURGERY_GROUNDED_SPECIALTY_EVIDENCE_MAP_TOOL, "arguments": {}}]}
            return {"answer": "The supplied map was rejected.", "unknowns": [], "claims": [{
                "claim_id": "map-rejected", "kind": "limitation", "scope": "public_record_metadata",
                "text": "A lane-mismatched specialist map cannot be used.",
                "citations": [{"record_kind": "genomic_project", "record_id": "TCGA-GBM"}],
            }]}

        runtime.register_in_memory_provider("ollama", local_handler, protocol="openai_chat_completions", structured_output_mode="json_object")
        result = agent.grounded_real_data_research(
            "Map the specialist glioma evidence coverage.",
            {"schema_version": "bioprism-neurosurgery-real/0.1", "synthetic_data": False},
            runtime, "ollama", "llama3.1", approve_provider_call=True, tool_loop=True,
            real_data_query={"limit": 1},
        )
        self.assertEqual(result["tool_trace"][0]["status"], "error")
        self.assertIn("fixed glioma lane", result["tool_trace"][0]["error"])

    def test_grounded_real_data_review_queue_rejects_unrepresentable_caller_facets(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        turns = 0

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal turns
            turns += 1
            if turns == 1:
                return {"tool_calls": [{"id": "queue-bad", "name": NEUROSURGERY_GROUNDED_REAL_REVIEW_QUEUE_TOOL,
                                         "arguments": {"max_items": 8}}]}
            return {"answer": "The caller-bound query remains available for review.", "unknowns": [], "claims": [{
                "claim_id": "queue-bound", "kind": "source_observation", "scope": "public_record_metadata",
                "text": "The caller supplied a structured trial facet.",
                "citations": [{"record_kind": "clinical_trial", "record_id": "NCT00000001"}],
            }]}

        runtime.register_in_memory_provider("ollama", local_handler, protocol="openai_chat_completions", structured_output_mode="json_object")
        result = agent.grounded_real_data_research(
            "Review trial metadata obligations.",
            {"schema_version": "bioprism-neurosurgery-real/0.1", "synthetic_data": False},
            runtime, "ollama", "llama3.1", approve_provider_call=True, tool_loop=True,
            real_data_query={"record_kind": "clinical_trial", "trial_phase": "PHASE2", "limit": 1},
        )
        self.assertEqual(result["tool_trace"][0]["status"], "error")
        self.assertIn("cannot combine caller facet trial_phase", result["tool_trace"][0]["error"])
        self.assertEqual([name for name, _ in client.calls], [
            "neurosurgery_real_data_reasoning_context", "neurosurgery_real_data_draft_audit",
        ])

    def test_grounded_real_data_tool_loop_rejects_structured_facet_override(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        turns = 0

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal turns
            turns += 1
            if turns == 1:
                return {"tool_calls": [{"id": "bad-facet", "name": "neurosurgery_real_data_search",
                                         "arguments": {"text": "genomic", "record_kind": "genomic_project"}}]}
            return {"answer": "The caller-bound trial context remains available.", "unknowns": [], "claims": [{
                "claim_id": "caller-bound", "kind": "source_observation", "scope": "public_record_metadata",
                "text": "The caller supplied a clinical-trial lane.",
                "citations": [{"record_kind": "clinical_trial", "record_id": "NCT00000001"}],
            }]}

        runtime.register_in_memory_provider("ollama", local_handler, protocol="openai_chat_completions", structured_output_mode="json_object")
        result = agent.grounded_real_data_research(
            "Find trial metadata.",
            {"schema_version": "bioprism-neurosurgery-real/0.1", "synthetic_data": False},
            runtime, "ollama", "llama3.1", approve_provider_call=True, tool_loop=True,
            real_data_query={"record_kind": "clinical_trial", "limit": 1},
        )
        self.assertEqual(result["tool_trace"][0]["status"], "error")
        self.assertIn("cannot override caller facet record_kind", result["tool_trace"][0]["error"])
        self.assertEqual([name for name, _ in client.calls], ["neurosurgery_real_data_reasoning_context", "neurosurgery_real_data_draft_audit"])

    def test_grounded_bridges_fail_closed_for_citations_omitted_from_model_context(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()

        runtime.register_in_memory_provider(
            "ollama",
            lambda request: {
                "answer": "The answer cites a record that was not supplied.",
                "unknowns": [],
                "claims": [
                    {
                        "claim_id": "out-of-context",
                        "kind": "source_observation",
                        "scope": "public_record_metadata",
                        "text": "This source identity was not present in the bounded context.",
                        "citations": [
                            {"record_kind": "guideline_reference", "record_id": "hidden-guideline"}
                        ],
                    }
                ],
            },
            protocol="openai_chat_completions",
            structured_output_mode="json_object",
        )
        bundle = {
            "schema_version": "bioprism-neurosurgery-real/0.1",
            "synthetic_data": False,
        }
        with self.assertRaises(ProtocolError):
            agent.grounded_real_data_research(
                "Summarize the bounded glioma metadata.",
                bundle,
                runtime,
                provider="ollama",
                model="llama3.1",
                approve_provider_call=True,
                include_abstracts=False,
            )
        self.assertEqual(
            [name for name, _ in client.calls],
            ["neurosurgery_real_data_reasoning_context"],
        )

    def test_grounded_literature_bridge_rejects_unseen_pmid(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        runtime.register_in_memory_provider(
            "ollama",
            lambda request: {
                "answer": "The answer cites an unseen PMID.",
                "unknowns": [],
                "claims": [
                    {
                        "claim_id": "unseen-pmid",
                        "kind": "source_observation",
                        "scope": "citation_metadata",
                        "text": "The hidden citation was not supplied in the context.",
                        "citations": [
                            {"record_kind": "literature_article", "record_id": "99999999"}
                        ],
                    }
                ],
            },
            protocol="openai_chat_completions",
            structured_output_mode="json_object",
        )
        with self.assertRaises(ProtocolError):
            agent.grounded_public_literature_research(
                "Summarize the bounded Chiari literature.",
                {
                    "schema_version": "bioprism-neurosurgery-public-literature/0.1",
                    "synthetic_data": False,
                },
                runtime,
                provider="ollama",
                model="llama3.1",
                specialty="chiari_malformation",
                approve_provider_call=True,
                include_abstracts=False,
            )
        self.assertEqual(
            [name for name, _ in client.calls],
            ["neurosurgery_public_literature_reasoning_context"],
        )

    def test_grounded_public_literature_bridge_covers_congenital_and_craniocervical_lanes(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()

        def local_handler(request: Any) -> Mapping[str, Any]:
            self.assertEqual(request.model, "llama3.1")
            return {
                "answer": "The selected lane contains source-linked Chiari literature for reviewer inspection.",
                "unknowns": ["The citation set does not establish an individual patient finding."],
                "claims": [
                    {
                        "claim_id": "chiari-literature",
                        "kind": "source_observation",
                        "scope": "citation_metadata",
                        "text": "The bounded PubMed packet contains a Chiari citation.",
                        "citations": [
                            {"record_kind": "literature_article", "record_id": "12345678"}
                        ],
                    }
                ],
            }

        runtime.register_in_memory_provider(
            "ollama",
            local_handler,
            protocol="openai_chat_completions",
            structured_output_mode="json_object",
        )
        bundle = {
            "schema_version": "bioprism-neurosurgery-public-literature/0.1",
            "synthetic_data": False,
        }
        grounded = agent.grounded_public_literature_research(
            "What source-linked Chiari literature is available?",
            bundle,
            runtime,
            provider="ollama",
            model="llama3.1",
            specialty="chiari_malformation",
            approve_provider_call=True,
            include_abstracts=False,
            freshness={"as_of": "2026-08-31T00:00:00Z", "max_age_days": 180},
        )
        self.assertEqual(
            grounded["schema_version"],
            "bioprism-neurosurgery-grounded-literature-research/0.1",
        )
        self.assertEqual(grounded["specialty"], "chiari_malformation")
        self.assertEqual(grounded["status"], "grounded_for_human_review")
        self.assertEqual(grounded["transport"], "in_memory")
        self.assertEqual(grounded["audit"]["status"], "grounded_for_human_review")
        self.assertTrue(grounded["human_review_required"])
        self.assertEqual(
            [name for name, _ in client.calls[-2:]],
            [
                "neurosurgery_public_literature_reasoning_context",
                "neurosurgery_public_literature_draft_audit",
            ],
        )
        self.assertEqual(
            client.calls[-2][1]["query"]["packet"]["query"]["specialty"],
            "chiari_malformation",
        )
        self.assertEqual(client.calls[-2][1]["query"]["packet"]["freshness"]["max_age_days"], 180)
        self.assertEqual(client.calls[-1][1]["query"]["freshness"]["max_age_days"], 180)

    def test_grounded_public_literature_tool_loop_supports_structured_facets(self) -> None:
        class ToolSearchClient(FakeClient):
            def call_tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> ToolResult:
                if name == "neurosurgery_public_literature_query":
                    args = dict(arguments or {})
                    self.calls.append((name, args))
                    return ToolResult(tool=name, envelope={"content": [{"type": "text", "text": json.dumps({
                        "schema_version": "bioprism-neurosurgery-public-literature/0.1",
                        "query": args["query"], "total_matches": 1, "returned_matches": 1,
                        "truncated": False,
                        "hits": [{"specialty": "chiari_malformation", "pmid": "FACET-PMID",
                                  "title": "Chiari review", "journal": "Neurosurgery",
                                  "source_id": "pubmed_chiari", "source_uri": "https://pubmed.ncbi.nlm.nih.gov/FACET-PMID/",
                                  "record_uri": "https://pubmed.ncbi.nlm.nih.gov/FACET-PMID/"}],
                    })}]})
                return super().call_tool(name, arguments)

        client = ToolSearchClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        turns = 0

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal turns
            turns += 1
            if turns == 1:
                tool_properties = request.tools[0].parameters["properties"]
                self.assertIn("mesh_term", tool_properties)
                self.assertNotIn("specialty", tool_properties)
                return {"tool_calls": [{"id": "literature-facet", "name": "neurosurgery_public_literature_search",
                                         "arguments": {"mesh_term": "Chiari Malformation", "limit": 128}}]}
            return {"answer": "The structured literature search returned one citation.", "unknowns": [], "claims": [{
                "claim_id": "facet-pmid", "kind": "source_observation", "scope": "citation_metadata",
                "text": "A bounded structured PubMed query returned one citation.",
                "citations": [{"record_kind": "literature_article", "record_id": "FACET-PMID"}],
            }]}

        runtime.register_in_memory_provider("ollama", local_handler, protocol="openai_chat_completions", structured_output_mode="json_object")
        result = agent.grounded_public_literature_research(
            "Find Chiari reviews.",
            {"schema_version": "bioprism-neurosurgery-public-literature/0.1", "synthetic_data": False},
            runtime, "ollama", "llama3.1", specialty="chiari_malformation", approve_provider_call=True,
            tool_loop=True, public_literature_query={"specialty": "chiari_malformation", "publication_type": "Review", "limit": 1},
        )
        query_call = next(args for name, args in client.calls if name == "neurosurgery_public_literature_query")
        self.assertEqual(query_call["query"]["specialty"], "chiari_malformation")
        self.assertEqual(query_call["query"]["publication_type"], "Review")
        self.assertEqual(query_call["query"]["mesh_term"], "Chiari Malformation")
        self.assertEqual(query_call["query"]["limit"], 1)
        self.assertEqual(result["tool_trace"][0]["query"]["mesh_term"], "Chiari Malformation")
        self.assertNotIn("text", result["tool_trace"][0]["query"])

    def test_grounded_public_literature_tool_loop_supports_integrity_review_queue(self) -> None:
        class QueueClient(FakeClient):
            def call_tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> ToolResult:
                if name == "neurosurgery_public_literature_review_queue":
                    args = dict(arguments or {})
                    self.calls.append((name, args))
                    return ToolResult(tool=name, envelope={"content": [{"type": "text", "text": json.dumps({
                        "schema_version": "bioprism-neurosurgery-public-literature-review-queue/0.1",
                        "bundle_digest": "p" * 64,
                        "queue_digest": "q" * 64,
                        "integrity_audit_digest": "i" * 64,
                        "generated_at": "2026-08-31T00:00:00Z",
                        "query": args["query"],
                        "candidate_item_count": 2,
                        "returned_item_count": 1,
                        "omitted_item_count": 1,
                        "omitted_integrity_issue_count": 0,
                        "truncated": True,
                        "items": [{
                            "task_id": "queue-task-1", "class": "completeness", "kind": "missing_abstract",
                            "status": "needs_human_review", "specialty": "chiari_malformation",
                            "source_id": "pubmed_chiari", "source_uri": "https://pubmed.ncbi.nlm.nih.gov/QUEUE-PMID/",
                            "pmid": "QUEUE-PMID", "record_uri": "https://pubmed.ncbi.nlm.nih.gov/QUEUE-PMID/",
                            "title": "A citation needing abstract review", "related_pmids": ["12345678"],
                            "reason": "abstract is absent from the checked-in snapshot", "reviewer_roles": ["neurosurgery"],
                            "patient_values": {"should": "never cross"},
                        }],
                        "provenance_bound": True, "synthetic_data": False, "human_review_required": True,
                        "provider": "none", "network": False, "effect": "read_only", "limitations": [],
                    })}]} )
                return super().call_tool(name, arguments)

        client = QueueClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        turns = 0

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal turns
            turns += 1
            if turns == 1:
                self.assertEqual(request.tools[1].name, NEUROSURGERY_GROUNDED_LITERATURE_REVIEW_QUEUE_TOOL)
                return {"tool_calls": [{"id": "queue-call", "name": NEUROSURGERY_GROUNDED_LITERATURE_REVIEW_QUEUE_TOOL, "arguments": {"max_items": 128}}]}
            return {"answer": "The corpus queue contains a bounded metadata task.", "unknowns": [], "claims": [{
                "claim_id": "queue-claim", "kind": "source_observation", "scope": "citation_metadata",
                "text": "One PubMed record is flagged for human abstract review.",
                "citations": [{"record_kind": "literature_article", "record_id": "QUEUE-PMID"}],
            }]}

        runtime.register_in_memory_provider("ollama", local_handler, protocol="openai_chat_completions", structured_output_mode="json_object")
        result = agent.grounded_public_literature_research(
            "Review Chiari corpus completeness.",
            {"schema_version": "bioprism-neurosurgery-public-literature/0.1", "synthetic_data": False},
            runtime, "ollama", "llama3.1", specialty="chiari_malformation", approve_provider_call=True,
            tool_loop=True, public_literature_query={"specialty": "chiari_malformation", "limit": 2},
        )
        queue_call = next(args for name, args in client.calls if name == "neurosurgery_public_literature_review_queue")
        self.assertEqual(queue_call["query"], {"specialties": ["chiari_malformation"], "max_items": 2})
        self.assertFalse(any(name == "neurosurgery_public_literature_query" for name, _ in client.calls))
        self.assertEqual(result["tool_trace"][0]["view"], "review_queue")
        self.assertEqual(result["tool_trace"][0]["queue_digest"], "q" * 64)
        self.assertEqual(result["claims"][0]["citations"][0]["record_id"], "QUEUE-PMID")
        self.assertTrue(result["human_review_required"])

    def test_grounded_public_literature_tool_loop_exposes_next_evidence_worklist(self) -> None:
        class AcquisitionClient(FakeClient):
            def call_tool(self, name: str, arguments: Mapping[str, Any] | None = None) -> ToolResult:
                if name == "neurosurgery_evidence_acquisition":
                    args = dict(arguments or {})
                    self.calls.append((name, args))
                    return ToolResult(tool=name, envelope={"content": [{"type": "text", "text": json.dumps({
                        "schema_version": "bioprism-neurosurgery-evidence-acquisition/0.1",
                        "plan_digest": "l" * 64, "request_digest": "r" * 64, "specialty": "chiari_malformation",
                        "query": args["query"], "audit": {},
                        "steps": [{
                            "sequence": 1, "step_id": "literature-step-1", "source": "public_literature",
                            "trigger": "missing_evidence_record", "observation_kind": "neuroanatomy",
                            "query": {"source": "public_literature", "query": {"specialty": "chiari_malformation", "limit": 2}},
                            "fallback_to_specialty_scan": False, "status": "candidates_found",
                            "total_matches": 2, "returned_matches": 2, "truncated": False,
                            "references": [{"source": "public_literature", "source_id": "pubmed_chiari", "record_id": "ACQ-PMID",
                                            "title": "A bounded Chiari citation", "uri": "https://pubmed.ncbi.nlm.nih.gov/ACQ-PMID/"}],
                        }],
                        "candidate_step_count": 1, "omitted_step_count": 0, "truncated": False,
                        "source_query_count": 1, "source_candidate_count": 2, "required_sources": ["public_literature"],
                        "ready_for_local_replay": True, "human_review_required": True, "provider": "none", "network": False,
                        "effect": "read_only", "limitations": ["local query worklist only"],
                    })}]} )
                return super().call_tool(name, arguments)

        client = AcquisitionClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        turns = 0

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal turns
            turns += 1
            if turns == 1:
                self.assertTrue(any(tool.name == NEUROSURGERY_GROUNDED_LITERATURE_EVIDENCE_ACQUISITION_TOOL for tool in request.tools))
                return {"tool_calls": [{"id": "literature-acquisition-view", "name": NEUROSURGERY_GROUNDED_LITERATURE_EVIDENCE_ACQUISITION_TOOL,
                                         "arguments": {"max_steps": 64, "max_references_per_step": 16}}]}
            return {"answer": "The bounded worker found a PubMed metadata query for reviewer replay.", "unknowns": [], "claims": [{
                "claim_id": "literature-acquisition-plan", "kind": "limitation", "scope": "citation_metadata",
                "text": "The next-evidence plan is a reviewer-owned local query, not a clinical finding.",
                "citations": [{"record_kind": "literature_article", "record_id": "ACQ-PMID"}],
            }]}

        runtime.register_in_memory_provider("ollama", local_handler, protocol="openai_chat_completions", structured_output_mode="json_object")
        result = agent.grounded_public_literature_research(
            "Find the next bounded Chiari literature evidence wave.",
            {"schema_version": "bioprism-neurosurgery-public-literature/0.1", "synthetic_data": False},
            runtime, "ollama", "llama3.1", specialty="chiari_malformation", approve_provider_call=True,
            tool_loop=True, public_literature_query={"specialty": "chiari_malformation", "limit": 2},
        )
        acquisition_call = next(args for name, args in client.calls if name == "neurosurgery_evidence_acquisition")
        self.assertEqual(acquisition_call["query"], {"max_steps": 2, "max_references_per_step": 16})
        self.assertEqual(result["tool_trace"][0]["view"], "evidence_acquisition")
        self.assertEqual(result["tool_trace"][0]["plan_digest"], "l" * 64)
        self.assertEqual(result["tool_trace"][0]["returned_steps"], 1)
        self.assertEqual(result["claims"][0]["citations"][0]["record_id"], "ACQ-PMID")
        self.assertTrue(result["human_review_required"])

    def test_grounded_public_literature_tool_loop_exposes_caller_clocked_freshness(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        turns = 0

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal turns
            turns += 1
            if turns == 1:
                self.assertTrue(any(tool.name == NEUROSURGERY_GROUNDED_LITERATURE_FRESHNESS_TOOL for tool in request.tools))
                return {"tool_calls": [{"id": "literature-freshness", "name": NEUROSURGERY_GROUNDED_LITERATURE_FRESHNESS_TOOL, "arguments": {"max_sources": 4}}]}
            return {"answer": "The caller-clocked literature snapshot is current for review.", "unknowns": [], "claims": [{
                "claim_id": "literature-freshness", "kind": "limitation", "scope": "citation_metadata",
                "text": "Source age remains a caller-clocked metadata state, not a clinical finding.",
                "citations": [{"record_kind": "literature_article", "record_id": "12345678"}],
            }]}

        runtime.register_in_memory_provider("ollama", local_handler, protocol="openai_chat_completions", structured_output_mode="json_object")
        result = agent.grounded_public_literature_research(
            "Check the freshness of the glioma literature snapshot.",
            {"schema_version": "bioprism-neurosurgery-public-literature/0.1", "synthetic_data": False},
            runtime, "ollama", "llama3.1", specialty="glioma", approve_provider_call=True,
            tool_loop=True, freshness={"as_of": "2026-08-31T00:00:00Z", "max_age_days": 180},
            public_literature_query={"specialty": "glioma", "limit": 1},
        )
        freshness_call = next(args for name, args in client.calls if name == "neurosurgery_public_literature_freshness")
        self.assertEqual(freshness_call["query"], {"as_of": "2026-08-31T00:00:00Z", "max_age_days": 180})
        self.assertEqual(result["tool_trace"][0]["view"], "freshness")
        self.assertEqual(result["tool_trace"][0]["freshness_digest"], "f" * 64)

    def test_grounded_public_literature_tool_loop_exposes_integrity_audit(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        turns = 0

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal turns
            turns += 1
            if turns == 1:
                self.assertTrue(any(tool.name == NEUROSURGERY_GROUNDED_LITERATURE_INTEGRITY_TOOL for tool in request.tools))
                return {"tool_calls": [{"id": "literature-integrity", "name": NEUROSURGERY_GROUNDED_LITERATURE_INTEGRITY_TOOL, "arguments": {"max_issues": 8}}]}
            return {"answer": "The PubMed snapshot has an explicit metadata integrity obligation.", "unknowns": [], "claims": [{
                "claim_id": "literature-integrity", "kind": "limitation", "scope": "citation_metadata",
                "text": "A missing abstract is a reviewer-owned metadata issue and is not negative evidence.",
                "citations": [{"record_kind": "literature_article", "record_id": "PMID-12345678"}],
            }]}

        runtime.register_in_memory_provider("ollama", local_handler, protocol="openai_chat_completions", structured_output_mode="json_object")
        result = agent.grounded_public_literature_research(
            "Audit integrity of the glioma literature snapshot.",
            {"schema_version": "bioprism-neurosurgery-public-literature/0.1", "synthetic_data": False},
            runtime, "ollama", "llama3.1", specialty="glioma", approve_provider_call=True,
            tool_loop=True, public_literature_query={"specialty": "glioma", "limit": 1},
        )
        integrity_call = next(args for name, args in client.calls if name == "neurosurgery_public_literature_integrity_audit")
        self.assertEqual(integrity_call["query"], {"max_issues": 8, "specialties": ["glioma"]})
        self.assertEqual(result["tool_trace"][0]["view"], "integrity")
        self.assertEqual(result["tool_trace"][0]["audit_digest"], "i" * 64)
        self.assertEqual(result["tool_trace"][0]["returned_issues"], 1)
        self.assertEqual(result["claims"][0]["citations"][0]["record_id"], "PMID-12345678")

    def test_grounded_public_literature_tool_loop_exposes_specialty_evidence_map(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        turns = 0

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal turns
            turns += 1
            if turns == 1:
                self.assertTrue(any(tool.name == NEUROSURGERY_GROUNDED_SPECIALTY_EVIDENCE_MAP_TOOL for tool in request.tools))
                return {"tool_calls": [{"id": "literature-map", "name": NEUROSURGERY_GROUNDED_SPECIALTY_EVIDENCE_MAP_TOOL, "arguments": {"max_dimensions": 4}}]}
            return {"answer": "The specialist map reports a bounded literature coverage hold.", "unknowns": [], "claims": [{
                "claim_id": "literature-map-hold", "kind": "limitation", "scope": "citation_metadata",
                "text": "The specialist map is reviewer planning metadata, not a clinical finding.",
                "citations": [{"record_kind": "literature_article", "record_id": "12345678"}],
            }]}

        runtime.register_in_memory_provider("ollama", local_handler, protocol="openai_chat_completions", structured_output_mode="json_object")
        result = agent.grounded_public_literature_research(
            "Map the specialist glioma literature coverage.",
            {"schema_version": "bioprism-neurosurgery-public-literature/0.1", "synthetic_data": False},
            runtime, "ollama", "llama3.1", specialty="glioma", approve_provider_call=True,
            tool_loop=True, public_literature_query={"specialty": "glioma", "limit": 1},
        )
        map_call = next(args for name, args in client.calls if name == "neurosurgery_specialty_evidence_map")
        self.assertEqual(map_call["request"]["specialty"], "glioma")
        self.assertEqual(result["tool_trace"][0]["view"], "specialty_evidence_map")
        self.assertEqual(result["tool_trace"][0]["map_digest"], "m" * 64)
        self.assertEqual(result["claims"][0]["citations"][0]["record_id"], "12345678")

    def test_grounded_public_literature_evidence_acquisition_rejects_caller_facets(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        turns = 0

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal turns
            turns += 1
            if turns == 1:
                return {"tool_calls": [{"id": "literature-acquisition-facet", "name": NEUROSURGERY_GROUNDED_LITERATURE_EVIDENCE_ACQUISITION_TOOL, "arguments": {}}]}
            return {"answer": "The acquisition request was constrained.", "unknowns": [], "claims": [{
                "claim_id": "context-claim", "kind": "source_observation", "scope": "citation_metadata",
                "text": "The supplied context remains the only citation source.",
                "citations": [{"record_kind": "literature_article", "record_id": "12345678"}],
            }]}

        runtime.register_in_memory_provider("ollama", local_handler, protocol="openai_chat_completions", structured_output_mode="json_object")
        result = agent.grounded_public_literature_research(
            "Find a bounded Chiari literature evidence wave.",
            {"schema_version": "bioprism-neurosurgery-public-literature/0.1", "synthetic_data": False},
            runtime, "ollama", "llama3.1", specialty="chiari_malformation", approve_provider_call=True,
            tool_loop=True, public_literature_query={"specialty": "chiari_malformation", "publication_type": "Review", "limit": 1},
        )
        self.assertEqual(result["tool_trace"][0]["status"], "error")
        self.assertIn("publication_type", result["tool_trace"][0]["error"])
        self.assertFalse(any(name == "neurosurgery_evidence_acquisition" for name, _ in client.calls))

    def test_grounded_public_literature_review_queue_rejects_caller_facets(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        turns = 0

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal turns
            turns += 1
            if turns == 1:
                return {"tool_calls": [{"id": "queue-facet", "name": NEUROSURGERY_GROUNDED_LITERATURE_REVIEW_QUEUE_TOOL, "arguments": {}}]}
            return {"answer": "The queue request was constrained.", "unknowns": [], "claims": [{
                "claim_id": "context-claim", "kind": "source_observation", "scope": "citation_metadata",
                "text": "The supplied context remains the only citation source.",
                "citations": [{"record_kind": "literature_article", "record_id": "12345678"}],
            }]}

        runtime.register_in_memory_provider("ollama", local_handler, protocol="openai_chat_completions", structured_output_mode="json_object")
        result = agent.grounded_public_literature_research(
            "Review Chiari corpus completeness.",
            {"schema_version": "bioprism-neurosurgery-public-literature/0.1", "synthetic_data": False},
            runtime, "ollama", "llama3.1", specialty="chiari_malformation", approve_provider_call=True,
            tool_loop=True, public_literature_query={"specialty": "chiari_malformation", "publication_type": "Review", "limit": 1},
        )
        self.assertEqual(result["tool_trace"][0]["status"], "error")
        self.assertIn("publication_type", result["tool_trace"][0]["error"])
        self.assertFalse(any(name == "neurosurgery_public_literature_review_queue" for name, _ in client.calls))

    def test_grounded_bridges_reject_credentialless_remote_http_provider(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        runtime.register_provider(
            ProviderConfig(
                provider="remote-no-key",
                base_url="https://gateway.example.invalid/v1",
                protocol="openai_chat_completions",
                requires_credential=False,
                structured_output_mode="json_object",
            )
        )
        real_bundle = {
            "schema_version": "bioprism-neurosurgery-real/0.1",
            "synthetic_data": False,
            "sources": [],
        }
        literature_bundle = {
            "schema_version": "bioprism-neurosurgery-public-literature/0.1",
            "synthetic_data": False,
            "sources": [],
            "records": [],
        }
        with self.assertRaises(ArgumentError):
            agent.grounded_real_data_research(
                "summarize",
                real_bundle,
                runtime,
                provider="remote-no-key",
                model="model",
                approve_provider_call=True,
            )
        with self.assertRaises(ArgumentError):
            agent.grounded_public_literature_research(
                "summarize",
                literature_bundle,
                runtime,
                provider="remote-no-key",
                model="model",
                approve_provider_call=True,
            )
        self.assertEqual(client.calls, [])

    def test_grounded_research_loops_expand_unknowns_and_terminate_at_bounded_review(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        calls = 0

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal calls
            calls += 1
            return {
                "answer": f"Pass {calls} is a source-bound observation.",
                "unknowns": ["verify missing publication linkage"] if calls == 1 else [],
                "claims": [
                    {
                        "claim_id": f"claim-{calls}",
                        "kind": "population_summary",
                        "scope": "population_aggregate",
                        "text": "The supplied public snapshot remains a population metadata source.",
                        "citations": [
                            {"record_kind": "genomic_project", "record_id": "TCGA-GBM"}
                        ],
                    }
                ],
            }

        runtime.register_in_memory_provider(
            "ollama",
            local_handler,
            protocol="openai_chat_completions",
            structured_output_mode="json_object",
        )
        bundle = {
            "schema_version": "bioprism-neurosurgery-real/0.1",
            "synthetic_data": False,
        }
        result = agent.grounded_real_data_research_loop(
            "Summarize the available glioma population metadata.",
            bundle,
            runtime,
            "ollama",
            "llama3.1",
            approve_provider_call=True,
            max_passes=3,
            max_follow_ups_per_pass=2,
            include_abstracts=False,
        )
        self.assertEqual(result["schema_version"], "bioprism-neurosurgery-grounded-research-loop/0.1")
        self.assertEqual(result["completed_pass_count"], 2)
        self.assertEqual(result["termination"], "no_new_queries")
        self.assertEqual(result["pending_queries"], [])
        self.assertEqual(result["claim_count"], 2)
        self.assertEqual(result["status"], "grounded_for_human_review")
        self.assertEqual(len(result["passes"][0]["follow_up_queries"]), 1)
        self.assertEqual(len(result["passes"][1]["follow_up_queries"]), 0)
        self.assertEqual(len(result["passes"][0]["claim_digest"]), 64)
        self.assertEqual(len(result["passes"][1]["claim_digest"]), 64)
        self.assertEqual(len(result["passes"][0]["audit_digest"]), 64)
        self.assertEqual(len(result["passes"][1]["audit_digest"]), 64)
        self.assertEqual(calls, 2)
        self.assertEqual(
            [name for name, _ in client.calls].count("neurosurgery_real_data_reasoning_context"),
            2,
        )
        self.assertEqual(
            [name for name, _ in client.calls].count("neurosurgery_real_data_draft_audit"),
            2,
        )

    def test_grounded_research_loops_resume_tamper_evident_pending_ledger(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        calls = 0

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal calls
            calls += 1
            return {
                "answer": f"Resumable pass {calls}.",
                "unknowns": ["check the source refresh timestamp"] if calls == 1 else [],
                "claims": [
                    {
                        "claim_id": f"resume-claim-{calls}",
                        "kind": "population_summary",
                        "scope": "population_aggregate",
                        "text": "The supplied snapshot is a population metadata source.",
                        "citations": [
                            {"record_kind": "genomic_project", "record_id": "TCGA-GBM"}
                        ],
                    }
                ],
            }

        runtime.register_in_memory_provider(
            "ollama",
            local_handler,
            protocol="openai_chat_completions",
            structured_output_mode="json_object",
        )
        bundle = {
            "schema_version": "bioprism-neurosurgery-real/0.1",
            "synthetic_data": False,
        }
        checkpoint = agent.grounded_real_data_research_loop(
            "Summarize the available glioma population metadata.",
            bundle,
            runtime,
            "ollama",
            "llama3.1",
            approve_provider_call=True,
            max_passes=1,
            max_follow_ups_per_pass=1,
            include_abstracts=False,
        )
        self.assertEqual(checkpoint["termination"], "max_passes_reached")
        self.assertEqual(len(checkpoint["pending_queries"]), 1)
        self.assertEqual(checkpoint["status"], "incomplete_budget")
        self.assertEqual(
            checkpoint["research_policy"],
            {
                "max_follow_ups_per_pass": 1,
                "max_output_tokens": 2048,
                "max_hits": 32,
                "max_chars": 24000,
                "include_abstracts": False,
                "freshness": None,
                "tool_loop": False,
                "max_tool_turns": 4,
                "max_tool_calls": 8,
            },
        )
        with self.assertRaises(ArgumentError):
            agent.grounded_real_data_research_loop(
                "Summarize the available glioma population metadata.",
                bundle,
                runtime,
                "ollama",
                "llama3.1",
                approve_provider_call=True,
                max_passes=2,
                max_follow_ups_per_pass=1,
                max_chars=12000,
                include_abstracts=False,
                resume_from=checkpoint,
            )
        self.assertEqual(calls, 1)
        resumed = agent.grounded_real_data_research_loop(
            "Summarize the available glioma population metadata.",
            bundle,
            runtime,
            "ollama",
            "llama3.1",
            approve_provider_call=True,
            max_passes=2,
            max_follow_ups_per_pass=1,
            include_abstracts=False,
            resume_from=checkpoint,
        )
        self.assertEqual(resumed["completed_pass_count"], 2)
        self.assertEqual(resumed["termination"], "no_new_queries")
        self.assertEqual(resumed["pending_queries"], [])
        self.assertEqual(resumed["status"], "grounded_for_human_review")
        self.assertEqual(calls, 2)
        with self.assertRaises(ArgumentError):
            agent.grounded_real_data_research_loop(
                "Summarize the available glioma population metadata.",
                bundle,
                runtime,
                "ollama",
                "llama3.1",
                approve_provider_call=True,
                max_passes=2,
                resume_from={**checkpoint, "loop_digest": "tampered"},
            )
        tampered_pass = {
            **checkpoint["passes"][0],
            "claims": [
                {**checkpoint["passes"][0]["claims"][0], "text": "tampered claim payload"}
            ],
        }
        with self.assertRaises(ArgumentError):
            agent.grounded_real_data_research_loop(
                "Summarize the available glioma population metadata.",
                bundle,
                runtime,
                "ollama",
                "llama3.1",
                approve_provider_call=True,
                max_passes=2,
                resume_from={**checkpoint, "passes": [tampered_pass]},
            )
        tampered_audit = json.loads(json.dumps(checkpoint))
        tampered_audit["passes"][0]["audit"]["grounded_claim_count"] += 1
        with self.assertRaises(ArgumentError):
            agent.grounded_real_data_research_loop(
                "Summarize the available glioma population metadata.",
                bundle,
                runtime,
                "ollama",
                "llama3.1",
                approve_provider_call=True,
                max_passes=2,
                max_follow_ups_per_pass=1,
                include_abstracts=False,
                resume_from=tampered_audit,
            )
        with self.assertRaises(ArgumentError):
            agent.grounded_real_data_research_loop(
                "Summarize the available glioma population metadata.",
                bundle,
                runtime,
                "ollama",
                "llama3.1",
                approve_provider_call=True,
                max_passes=2,
                max_follow_ups_per_pass=1,
                include_abstracts=False,
                resume_from={**checkpoint, "grounded_claim_count": 999},
            )

    def test_grounded_real_data_loop_binds_query_facets_and_rejects_resume_drift(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()

        runtime.register_in_memory_provider(
            "ollama",
            lambda request: {
                "answer": "The filtered registry metadata is a source-bound observation.",
                "unknowns": [],
                "claims": [
                    {
                        "claim_id": "filtered-trials",
                        "kind": "population_summary",
                        "scope": "public_record_metadata",
                        "text": "The selected trial slice is limited to interventional studies.",
                        "citations": [
                            {"record_kind": "clinical_trial", "record_id": "NCT00000001"}
                        ],
                    }
                ],
            },
            protocol="openai_chat_completions",
            structured_output_mode="json_object",
        )
        bundle = {
            "schema_version": "bioprism-neurosurgery-real/0.1",
            "synthetic_data": False,
        }
        query = {
            "record_kind": "clinical_trial",
            "trial_study_type": "Interventional",
            "trial_updated_from": "2024-01-01",
            "trial_updated_to": "2024-12-31",
            "limit": 2,
        }
        checkpoint = agent.grounded_real_data_research_loop(
            "Summarize interventional glioma trials.",
            bundle,
            runtime,
            "ollama",
            "llama3.1",
            approve_provider_call=True,
            max_passes=1,
            real_data_query=query,
            include_abstracts=False,
        )
        self.assertEqual(
            checkpoint["real_data_query"],
            {
                **query,
                "text": "Summarize interventional glioma trials.",
            },
        )
        context_call = next(
            args for name, args in client.calls
            if name == "neurosurgery_real_data_reasoning_context"
        )
        self.assertEqual(context_call["query"]["packet"]["query"], checkpoint["real_data_query"])
        with self.assertRaises(ArgumentError):
            agent.grounded_real_data_research_loop(
                "Summarize interventional glioma trials.",
                bundle,
                runtime,
                "ollama",
                "llama3.1",
                approve_provider_call=True,
                max_passes=2,
                real_data_query={**query, "trial_study_type": "Observational"},
                resume_from=checkpoint,
            )

    def test_grounded_real_data_loop_executes_follow_up_text_with_explicit_facets(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        calls = 0

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal calls
            calls += 1
            return {
                "answer": f"Pass {calls}.",
                "unknowns": ["confirm linked publication metadata"] if calls == 1 else [],
                "claims": [
                    {
                        "claim_id": f"facet-follow-up-{calls}",
                        "kind": "source_observation",
                        "scope": "public_record_metadata",
                        "text": "The bounded source context remains metadata-only.",
                        "citations": [
                            {
                                "record_kind": "portal_molecular_profile",
                                "record_id": "profile-1",
                            }
                        ],
                    }
                ],
            }

        runtime.register_in_memory_provider(
            "ollama",
            local_handler,
            protocol="openai_chat_completions",
            structured_output_mode="json_object",
        )
        bundle = {
            "schema_version": "bioprism-neurosurgery-real/0.1",
            "synthetic_data": False,
        }
        query = {
            "text": "IDH molecular profile",
            "record_kind": "portal_molecular_profile",
            "molecular_alteration_type": "MUTATION_EXTENDED",
            "limit": 1,
        }
        result = agent.grounded_real_data_research_loop(
            "Summarize glioma molecular metadata.",
            bundle,
            runtime,
            "ollama",
            "llama3.1",
            approve_provider_call=True,
            max_passes=2,
            max_follow_ups_per_pass=1,
            include_abstracts=False,
            real_data_query=query,
        )
        context_queries = [
            args["query"]["packet"]["query"]
            for name, args in client.calls
            if name == "neurosurgery_real_data_reasoning_context"
        ]
        self.assertEqual(len(context_queries), 2)
        self.assertEqual(context_queries[0]["text"], query["text"])
        self.assertEqual(
            context_queries[1]["text"],
            "evidence metadata gap: confirm linked publication metadata",
        )
        self.assertEqual(
            context_queries[0]["record_kind"], context_queries[1]["record_kind"]
        )
        self.assertEqual(result["completed_pass_count"], 2)
        self.assertEqual(calls, 2)

    def test_grounded_literature_loops_preserve_specialty_and_pmid_audit_boundaries(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        calls = 0

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal calls
            calls += 1
            return {
                "answer": "The bounded PubMed lane remains a citation handoff.",
                "unknowns": ["verify abstract availability"] if calls == 1 else [],
                "claims": [
                    {
                        "claim_id": f"literature-claim-{calls}",
                        "kind": "source_observation",
                        "scope": "citation_metadata",
                        "text": "The selected specialty lane contains source-linked metadata.",
                        "citations": [
                            {"record_kind": "literature_article", "record_id": "12345678"}
                        ],
                    }
                ],
            }

        runtime.register_in_memory_provider(
            "ollama",
            local_handler,
            protocol="openai_chat_completions",
            structured_output_mode="json_object",
        )
        bundle = {
            "schema_version": "bioprism-neurosurgery-public-literature/0.1",
            "synthetic_data": False,
            "sources": [],
            "records": [],
        }
        result = agent.grounded_public_literature_research_loop(
            "Summarize source-linked Chiari literature.",
            bundle,
            runtime,
            "ollama",
            "llama3.1",
            specialty="chiari_malformation",
            approve_provider_call=True,
            max_passes=2,
            max_follow_ups_per_pass=1,
            include_abstracts=False,
        )
        self.assertEqual(
            result["schema_version"],
            "bioprism-neurosurgery-grounded-literature-research-loop/0.1",
        )
        self.assertEqual(result["specialty"], "chiari_malformation")
        self.assertEqual(result["completed_pass_count"], 2)
        self.assertEqual(result["termination"], "no_new_queries")
        self.assertEqual(result["status"], "grounded_for_human_review")
        self.assertEqual(calls, 2)
        self.assertEqual(
            [name for name, _ in client.calls].count("neurosurgery_public_literature_reasoning_context"),
            2,
        )
        self.assertEqual(
            [name for name, _ in client.calls].count("neurosurgery_public_literature_draft_audit"),
            2,
        )

    def test_grounded_literature_loop_preserves_structured_pubmed_facets_and_resume_identity(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        calls = 0

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal calls
            calls += 1
            return {
                "answer": f"Pass {calls}.",
                "unknowns": ["check date-bounded review coverage"] if calls == 1 else [],
                "claims": [{
                    "claim_id": f"facet-literature-{calls}",
                    "kind": "source_observation",
                    "scope": "citation_metadata",
                    "text": "The bounded PubMed context remains source-linked metadata.",
                    "citations": [{"record_kind": "literature_article", "record_id": "12345678"}],
                }],
            }

        runtime.register_in_memory_provider(
            "ollama", local_handler, protocol="openai_chat_completions", structured_output_mode="json_object"
        )
        bundle = {
            "schema_version": "bioprism-neurosurgery-public-literature/0.1",
            "synthetic_data": False,
            "sources": [],
            "records": [],
        }
        query = {
            "specialty": "glioma",
            "text": "IDH glioma reviews",
            "publication_type": "Review",
            "mesh_term": "Glioma",
            "from_date": "2020-01-01",
            "to_date": "2024-12-31",
            "limit": 7,
        }
        checkpoint = agent.grounded_public_literature_research_loop(
            "Summarize glioma evidence.",
            bundle,
            runtime,
            "ollama",
            "llama3.1",
            approve_provider_call=True,
            max_passes=1,
            max_follow_ups_per_pass=1,
            include_abstracts=False,
            public_literature_query=query,
        )
        self.assertEqual(checkpoint["termination"], "max_passes_reached")
        self.assertEqual(checkpoint["status"], "incomplete_budget")
        self.assertEqual(len(checkpoint["pending_queries"]), 1)
        resumed = agent.grounded_public_literature_research_loop(
            "Summarize glioma evidence.",
            bundle,
            runtime,
            "ollama",
            "llama3.1",
            approve_provider_call=True,
            max_passes=2,
            max_follow_ups_per_pass=1,
            include_abstracts=False,
            public_literature_query=query,
            resume_from=checkpoint,
        )
        self.assertEqual(resumed["status"], "grounded_for_human_review")
        self.assertEqual(resumed["pending_queries"], [])
        context_queries = [
            args["query"]["packet"]["query"]
            for name, args in client.calls
            if name == "neurosurgery_public_literature_reasoning_context"
        ]
        self.assertEqual(context_queries[0], query)
        self.assertEqual(context_queries[1]["publication_type"], "Review")
        self.assertEqual(context_queries[1]["mesh_term"], "Glioma")
        self.assertEqual(context_queries[1]["from_date"], "2020-01-01")
        self.assertEqual(context_queries[1]["to_date"], "2024-12-31")
        self.assertEqual(context_queries[1]["text"], "evidence metadata gap: check date-bounded review coverage")
        self.assertEqual(checkpoint["public_literature_query"], query)
        self.assertEqual(len(checkpoint["loop_digest"]), 64)
        with self.assertRaises(ArgumentError):
            agent.grounded_public_literature_research_loop(
                "Summarize glioma evidence.",
                bundle,
                runtime,
                "ollama",
                "llama3.1",
                approve_provider_call=True,
                max_passes=2,
                public_literature_query={**query, "mesh_term": "Glioblastoma"},
                resume_from=checkpoint,
            )

    def test_grounded_research_portfolio_coordinates_source_planes_without_blending(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        calls = 0

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal calls
            calls += 1
            return {
                "answer": "Source-separated portfolio handoff.",
                "unknowns": [],
                "claims": [
                    {
                        "claim_id": f"portfolio-claim-{calls}",
                        "kind": "source_observation",
                        "scope": "citation_metadata",
                        "text": "The selected source plane contains a bounded public record.",
                        "citations": [
                            {
                                "record_kind": "genomic_project" if calls == 1 else "literature_article",
                                "record_id": "TCGA-GBM" if calls == 1 else "12345678",
                            }
                        ],
                    }
                ],
            }

        runtime.register_in_memory_provider(
            "ollama",
            local_handler,
            protocol="openai_chat_completions",
            structured_output_mode="json_object",
        )
        result = agent.grounded_research_portfolio(
            "Summarize source-linked glioma evidence.",
            runtime,
            "ollama",
            "llama3.1",
            real_glioma_data={
                "schema_version": "bioprism-neurosurgery-real/0.1",
                "synthetic_data": False,
                "sources": [],
            },
            public_literature={
                "schema_version": "bioprism-neurosurgery-public-literature/0.1",
                "synthetic_data": False,
                "sources": [],
                "records": [],
            },
            specialty="glioma",
            approve_provider_call=True,
            max_passes=1,
            max_follow_ups_per_pass=0,
            include_abstracts=False,
            real_data_query={
                "record_kind": "genomic_project",
                "genomic_data_type": "Annotated Somatic Mutation",
                "limit": 1,
            },
        )
        self.assertEqual(
            result["schema_version"],
            "bioprism-neurosurgery-grounded-research-portfolio/0.1",
        )
        self.assertEqual(
            result["source_planes"], ["real_glioma_population", "public_literature"]
        )
        self.assertEqual(result["real_data_loop"]["bundle_digest"], "f" * 64)
        self.assertEqual(
            result["real_data_query"],
            {
                "record_kind": "genomic_project",
                "genomic_data_type": "Annotated Somatic Mutation",
                "limit": 1,
                "text": "Summarize source-linked glioma evidence.",
            },
        )
        self.assertEqual(result["public_literature_loop"]["bundle_digest"], "f" * 64)
        self.assertEqual(result["literature_link_audit"]["audit_digest"], "l" * 64)
        self.assertEqual(
            result["literature_link_audit"]["query"],
            {"public_specialty": "glioma", "max_links": 32, "max_unmatched_ids": 32},
        )
        self.assertEqual(result["completed_pass_count"], 2)
        self.assertEqual(result["claim_count"], 2)
        self.assertEqual(result["status"], "grounded_for_human_review")
        self.assertTrue(result["human_review_required"])
        self.assertEqual(calls, 2)

    def test_grounded_research_portfolio_refuses_a_synthetic_link_audit(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        runtime.register_in_memory_provider(
            "ollama",
            lambda request: {
                "answer": "bounded source handoff",
                "unknowns": [],
                "claims": [
                    {
                        "claim_id": "link-boundary",
                        "kind": "source_observation",
                        "scope": "citation_metadata",
                        "text": "A bounded source record is available for review.",
                        "citations": [
                            {"record_kind": "literature_article", "record_id": "12345678"}
                        ],
                    }
                ],
            },
            protocol="openai_chat_completions",
            structured_output_mode="json_object",
        )
        agent.literature_link_audit = lambda *args, **kwargs: {  # type: ignore[method-assign]
            "synthetic_data": True,
            "network": False,
            "provenance_bound": True,
            "human_review_required": True,
            "provider": "none",
            "effect": "read_only",
        }
        bundle = {
            "schema_version": "bioprism-neurosurgery-real/0.1",
            "synthetic_data": False,
            "sources": [],
        }
        literature = {
            "schema_version": "bioprism-neurosurgery-public-literature/0.1",
            "synthetic_data": False,
            "sources": [],
            "records": [],
        }
        with self.assertRaises(ProtocolError):
            agent.grounded_research_portfolio(
                "Summarize source-linked glioma evidence.",
                runtime,
                "ollama",
                "llama3.1",
                real_glioma_data=bundle,
                public_literature=literature,
                specialty="glioma",
                approve_provider_call=True,
                max_passes=1,
                max_follow_ups_per_pass=0,
                include_abstracts=False,
            )

    def test_grounded_research_portfolio_carries_deidentified_case_asset_projection(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        runtime.register_in_memory_provider(
            "ollama",
            lambda request: {
                "answer": "bounded real-data handoff",
                "unknowns": [],
                "claims": [
                    {
                        "claim_id": "case-asset-handoff",
                        "kind": "source_observation",
                        "scope": "population_aggregate",
                        "text": "The source snapshot exposes a bounded aggregate project record.",
                        "citations": [
                            {"record_kind": "genomic_project", "record_id": "TCGA-GBM"}
                        ],
                    }
                ],
            },
            protocol="openai_chat_completions",
            structured_output_mode="json_object",
        )
        result = agent.grounded_research_portfolio(
            "Summarize the real glioma metadata and attached case inventory.",
            runtime,
            "ollama",
            "llama3.1",
            real_glioma_data={
                "schema_version": "bioprism-neurosurgery-real/0.1",
                "synthetic_data": False,
                "sources": [],
            },
            specialty="glioma",
            case_asset_manifest={
                "schema_version": "bioprism-neurosurgery-case-asset-manifest/0.1",
                "specialty": "glioma",
                "synthetic_data": False,
                "assets": [],
            },
            case_asset_manifest_query={
                "requested_kinds": ["imaging_series"],
                "max_review_items": 16,
            },
            approve_provider_call=True,
            max_passes=1,
            max_follow_ups_per_pass=0,
            include_abstracts=False,
        )
        self.assertEqual(result["case_asset_manifest"]["report_digest"], "d" * 64)
        self.assertEqual(
            result["case_asset_manifest_query"],
            {"requested_kinds": ["imaging_series"], "max_review_items": 16},
        )
        self.assertEqual(
            [name for name, _ in client.calls if name == "neurosurgery_case_asset_manifest"],
            ["neurosurgery_case_asset_manifest"],
        )

    def test_grounded_research_intake_carries_deidentified_case_asset_projection(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        runtime.register_in_memory_provider(
            "ollama",
            lambda request: {
                "answer": "bounded real-data handoff",
                "unknowns": [],
                "claims": [{
                    "claim_id": "intake-case-asset-handoff",
                    "kind": "source_observation",
                    "scope": "population_aggregate",
                    "text": "The source snapshot exposes a bounded aggregate project record.",
                    "citations": [{"record_kind": "genomic_project", "record_id": "TCGA-GBM"}],
                }],
            },
            protocol="openai_chat_completions",
            structured_output_mode="json_object",
        )
        result = agent.grounded_research_intake(
            "What does the glioma molecular evidence contain?",
            runtime,
            "ollama",
            "llama3.1",
            real_glioma_data={
                "schema_version": "bioprism-neurosurgery-real/0.1",
                "synthetic_data": False,
                "sources": [],
            },
            case_asset_manifest={
                "schema_version": "bioprism-neurosurgery-case-asset-manifest/0.1",
                "specialty": "glioma",
                "synthetic_data": False,
                "assets": [],
            },
            case_asset_manifest_query={"requested_kinds": ["imaging_series"], "max_review_items": 16},
            approve_provider_call=True,
            max_passes=1,
            max_follow_ups_per_pass=0,
            include_abstracts=False,
        )
        self.assertEqual(result["status"], "grounded_for_human_review")
        real_context_call = next(
            args
            for name, args in client.calls
            if name == "neurosurgery_real_data_reasoning_context"
        )
        self.assertEqual(
            real_context_call["query"]["packet"]["query"]["text"],
            "glioma",
        )
        self.assertNotIn(
            "What does the glioma molecular evidence contain?",
            real_context_call["query"]["packet"]["query"]["text"],
        )
        self.assertEqual(result["portfolio"]["case_asset_manifest"]["report_digest"], "d" * 64)
        self.assertEqual(
            result["portfolio"]["case_asset_manifest_query"],
            {"requested_kinds": ["imaging_series"], "max_review_items": 16},
        )
        self.assertEqual(
            [name for name, _ in client.calls if name == "neurosurgery_case_asset_manifest"],
            ["neurosurgery_case_asset_manifest"],
        )

    def test_grounded_research_intake_gates_missing_real_glioma_snapshot_before_model(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        provider_calls = 0

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal provider_calls
            provider_calls += 1
            return {"answer": "must not run", "unknowns": [], "claims": []}

        runtime.register_in_memory_provider(
            "ollama",
            local_handler,
            protocol="openai_chat_completions",
            structured_output_mode="json_object",
        )
        result = agent.grounded_research_intake(
            "What does the glioma molecular evidence contain?",
            runtime,
            "ollama",
            "llama3.1",
            approve_provider_call=True,
        )
        self.assertEqual(result["status"], "needs_evidence")
        self.assertEqual(result["routed_specialty"], "glioma")
        self.assertEqual(result["required_evidence"], ["real_glioma_snapshot"])
        self.assertIsNone(result["portfolio"])
        self.assertEqual(provider_calls, 0)
        self.assertEqual(
            [name for name, _ in client.calls],
            ["neurosurgery_intake_plan"],
        )

    def test_grounded_research_intake_routes_non_glioma_to_public_plane_only(self) -> None:
        client = FakeClient()
        agent = LocalNeurosurgicalAgent(client)
        runtime = LLMRuntime()
        provider_calls = 0

        def local_handler(request: Any) -> Mapping[str, Any]:
            nonlocal provider_calls
            provider_calls += 1
            return {
                "answer": "Source-separated congenital literature handoff.",
                "unknowns": ["verify the unreported follow-up horizon"],
                "claims": [
                    {
                        "claim_id": "chiari-intake-claim",
                        "kind": "source_observation",
                        "scope": "citation_metadata",
                        "text": "The selected public literature lane contains a bounded citation record.",
                        "citations": [{"record_kind": "literature_article", "record_id": "12345678"}],
                    }
                ],
            }

        runtime.register_in_memory_provider(
            "ollama",
            local_handler,
            protocol="openai_chat_completions",
            structured_output_mode="json_object",
        )
        agent.intake_plan = lambda question, *, specialty=None, max_candidates=6: {  # type: ignore[method-assign]
            "schema_version": "bioprism-neurosurgery-intake-plan/0.1",
            "plan_digest": "i" * 64,
            "question_digest": "q" * 64,
            "candidates": [{"specialty": "chiari_malformation", "score_bps": 1000, "matched_terms": ["chiari"]}],
            "selected_specialty": "chiari_malformation",
            "confidence_bps": 1000,
            "abstained": False,
            "reason": "selected",
            "route": ["safety_gate", "public_literature", "human_review_hold"],
            "evidence_sources": ["pubmed_snapshot"],
            "reviewer_roles": ["neurosurgery"],
            "next_actions": [],
            "human_review_required": True,
            "provider": "none",
            "network": False,
            "effect": "read_only",
            "limitations": [],
        }
        result = agent.grounded_research_intake(
            "What source-linked Chiari literature is available?",
            runtime,
            "ollama",
            "llama3.1",
            public_literature={
                "schema_version": "bioprism-neurosurgery-public-literature/0.1",
                "synthetic_data": False,
                "sources": [],
                "records": [],
            },
            approve_provider_call=True,
            max_passes=1,
            max_follow_ups_per_pass=1,
            include_abstracts=False,
        )
        self.assertEqual(result["status"], "incomplete_budget")
        self.assertEqual(result["routed_specialty"], "chiari_malformation")
        self.assertEqual(result["source_planes"], ["public_literature"])
        self.assertIsNotNone(result["portfolio"])
        self.assertEqual(result["portfolio"]["status"], "incomplete_budget")
        self.assertEqual(
            result["portfolio"]["public_literature_loop"]["pending_queries"],
            ["evidence metadata gap: verify the unreported follow-up horizon"],
        )
        self.assertEqual(provider_calls, 1)
        self.assertNotIn("neurosurgery_real_data_reasoning_context", [name for name, _ in client.calls])
        self.assertIn("neurosurgery_public_literature_reasoning_context", [name for name, _ in client.calls])

    def test_context_manager_only_closes_facade_owned_clients(self) -> None:
        caller_owned = LifecycleClient()
        with LocalNeurosurgicalAgent(caller_owned):
            self.assertFalse(caller_owned.connected)
        self.assertFalse(caller_owned.closed)

        owned = LocalNeurosurgicalAgent(LifecycleClient())
        owned._owns_client = True
        with owned as active:
            self.assertIs(active, owned)
            self.assertTrue(owned.client.connected)
        self.assertTrue(owned.client.closed)


if __name__ == "__main__":
    unittest.main()
