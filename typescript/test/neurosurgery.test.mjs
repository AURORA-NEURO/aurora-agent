import assert from "node:assert/strict";
import test from "node:test";
import {
  ArgumentError,
  ProtocolError,
  LLMRuntime,
  LocalNeurosurgicalAgent,
  openaiCompatibleProvider,
  NEUROSURGERY_CATALOGUE_TOOL,
  NEUROSURGERY_INTAKE_PLAN_TOOL,
  NEUROSURGERY_INTAKE_MISSION_TOOL,
  NEUROSURGERY_INTAKE_PORTFOLIO_TOOL,
  NEUROSURGERY_EVIDENCE_AUDIT_TOOL,
  NEUROSURGERY_SPECIALTY_EVIDENCE_MAP_TOOL,
  NEUROSURGERY_EVIDENCE_PROGRAM_TOOL,
  NEUROSURGERY_CASE_ASSET_MANIFEST_TOOL,
  NEUROSURGERY_CASE_FHIR_IMPORT_TOOL,
  NEUROSURGERY_CASE_DICOM_IMPORT_TOOL,
  NEUROSURGERY_CASE_DICOM_EVIDENCE_WORKFLOW_TOOL,
  NEUROSURGERY_CASE_ASSET_REVIEW_DISPOSITION_TOOL,
  NEUROSURGERY_EVIDENCE_SYNTHESIS_TOOL,
  NEUROSURGERY_EVIDENCE_GRAPH_TOOL,
  NEUROSURGERY_GLIOMA_MOLECULAR_MAP_TOOL,
  NEUROSURGERY_REAL_DATA_COVERAGE_TOOL,
  NEUROSURGERY_REAL_DATA_COHORT_LANDSCAPE_TOOL,
  NEUROSURGERY_REAL_DATA_RECONCILIATION_TOOL,
  NEUROSURGERY_REAL_DATA_FRESHNESS_TOOL,
  NEUROSURGERY_REAL_DATA_DIFF_TOOL,
  NEUROSURGERY_REAL_DATA_REFRESH_AUDIT_TOOL,
  NEUROSURGERY_REAL_DATA_REVIEW_QUEUE_TOOL,
  NEUROSURGERY_REAL_DATA_REVIEW_DISPOSITION_TOOL,
  NEUROSURGERY_REAL_DATA_EVIDENCE_PACKET_TOOL,
  NEUROSURGERY_REAL_DATA_AUTONOMOUS_WORKFLOW_TOOL,
  NEUROSURGERY_REAL_DATA_REASONING_CONTEXT_TOOL,
  NEUROSURGERY_REAL_DATA_DRAFT_AUDIT_TOOL,
  NEUROSURGERY_PUBLIC_LITERATURE_EVIDENCE_PACKET_TOOL,
  NEUROSURGERY_PUBLIC_LITERATURE_REASONING_CONTEXT_TOOL,
  NEUROSURGERY_PUBLIC_LITERATURE_DRAFT_AUDIT_TOOL,
  NEUROSURGERY_PUBLIC_LITERATURE_MATRIX_TOOL,
  NEUROSURGERY_PUBLIC_LITERATURE_FRESHNESS_TOOL,
  NEUROSURGERY_PUBLIC_LITERATURE_REFRESH_AUDIT_TOOL,
  NEUROSURGERY_LITERATURE_LINK_AUDIT_TOOL,
  NEUROSURGERY_PUBLIC_LITERATURE_INTEGRITY_AUDIT_TOOL,
  NEUROSURGERY_PUBLIC_LITERATURE_REVIEW_QUEUE_TOOL,
  NEUROSURGERY_PUBLIC_LITERATURE_WORKBENCH_TOOL,
  NEUROSURGERY_PUBLIC_LITERATURE_PORTFOLIO_TOOL,
  NEUROSURGERY_RESEARCH_PLAN_TOOL,
  NEUROSURGERY_EVIDENCE_ACQUISITION_TOOL,
  NEUROSURGERY_RESEARCH_BRIEF_TOOL,
  NEUROSURGERY_REAL_DATA_QUERY_TOOL,
  NEUROSURGERY_REAL_DATA_TRIAL_LANDSCAPE_TOOL,
  NEUROSURGERY_REAL_DATA_MOLECULAR_COVERAGE_TOOL,
  NEUROSURGERY_PUBLIC_LITERATURE_QUERY_TOOL,
  NEUROSURGERY_GROUNDED_REAL_DATA_PROVIDER_TOOL,
  NEUROSURGERY_GROUNDED_LITERATURE_PROVIDER_TOOL,
  NEUROSURGERY_GROUNDED_REAL_TRIAL_LANDSCAPE_TOOL,
  NEUROSURGERY_GROUNDED_REAL_MOLECULAR_COVERAGE_TOOL,
  NEUROSURGERY_GROUNDED_REAL_COHORT_LANDSCAPE_TOOL,
  NEUROSURGERY_GROUNDED_REAL_RECONCILIATION_TOOL,
  NEUROSURGERY_GROUNDED_REAL_RESEARCH_BRIEF_TOOL,
  NEUROSURGERY_GROUNDED_REAL_REVIEW_QUEUE_TOOL,
  NEUROSURGERY_GROUNDED_REAL_EVIDENCE_GRAPH_TOOL,
  NEUROSURGERY_GROUNDED_REAL_EVIDENCE_ACQUISITION_TOOL,
  NEUROSURGERY_GROUNDED_REAL_COVERAGE_TOOL,
  NEUROSURGERY_GROUNDED_SPECIALTY_EVIDENCE_MAP_TOOL,
  NEUROSURGERY_GROUNDED_REAL_FRESHNESS_TOOL,
  NEUROSURGERY_GROUNDED_LITERATURE_FRESHNESS_TOOL,
  NEUROSURGERY_GROUNDED_LITERATURE_REVIEW_QUEUE_TOOL,
  NEUROSURGERY_GROUNDED_LITERATURE_INTEGRITY_TOOL,
  NEUROSURGERY_GROUNDED_LITERATURE_EVIDENCE_ACQUISITION_TOOL,
  NEUROSURGERY_MISSION_TOOL,
  NEUROSURGERY_GROUNDED_RESEARCH_PORTFOLIO_SCHEMA,
  NEUROSURGERY_GROUNDED_RESEARCH_INTAKE_SCHEMA,
  NEUROSURGERY_SESSION_TERMINAL_STATUS,
  NEUROSURGERY_SESSION_TOOL,
  NEUROSURGERY_TOOL,
} from "../dist/index.js";

function response(tool, value) {
  return {
    ok: true,
    tool,
    request_id: `${tool}-request`,
    mcp: { result: { structuredContent: value } },
    guarantee: "read-only research boundary",
  };
}

function fakeClient() {
  const calls = [];
  const definitions = [
    NEUROSURGERY_CATALOGUE_TOOL,
    NEUROSURGERY_INTAKE_PLAN_TOOL,
    NEUROSURGERY_INTAKE_MISSION_TOOL,
    NEUROSURGERY_INTAKE_PORTFOLIO_TOOL,
    NEUROSURGERY_EVIDENCE_AUDIT_TOOL,
    NEUROSURGERY_SPECIALTY_EVIDENCE_MAP_TOOL,
    NEUROSURGERY_EVIDENCE_PROGRAM_TOOL,
    NEUROSURGERY_CASE_ASSET_MANIFEST_TOOL,
    NEUROSURGERY_CASE_FHIR_IMPORT_TOOL,
    NEUROSURGERY_CASE_DICOM_IMPORT_TOOL,
    NEUROSURGERY_CASE_DICOM_EVIDENCE_WORKFLOW_TOOL,
    NEUROSURGERY_CASE_ASSET_REVIEW_DISPOSITION_TOOL,
    NEUROSURGERY_EVIDENCE_SYNTHESIS_TOOL,
    NEUROSURGERY_EVIDENCE_GRAPH_TOOL,
    NEUROSURGERY_GLIOMA_MOLECULAR_MAP_TOOL,
    NEUROSURGERY_REAL_DATA_COVERAGE_TOOL,
    NEUROSURGERY_REAL_DATA_COHORT_LANDSCAPE_TOOL,
    NEUROSURGERY_REAL_DATA_RECONCILIATION_TOOL,
    NEUROSURGERY_REAL_DATA_FRESHNESS_TOOL,
    NEUROSURGERY_REAL_DATA_DIFF_TOOL,
    NEUROSURGERY_REAL_DATA_REFRESH_AUDIT_TOOL,
    NEUROSURGERY_REAL_DATA_REVIEW_QUEUE_TOOL,
    NEUROSURGERY_REAL_DATA_REVIEW_DISPOSITION_TOOL,
    NEUROSURGERY_REAL_DATA_EVIDENCE_PACKET_TOOL,
    NEUROSURGERY_REAL_DATA_AUTONOMOUS_WORKFLOW_TOOL,
    NEUROSURGERY_REAL_DATA_REASONING_CONTEXT_TOOL,
    NEUROSURGERY_REAL_DATA_DRAFT_AUDIT_TOOL,
    NEUROSURGERY_PUBLIC_LITERATURE_EVIDENCE_PACKET_TOOL,
    NEUROSURGERY_PUBLIC_LITERATURE_REASONING_CONTEXT_TOOL,
    NEUROSURGERY_PUBLIC_LITERATURE_DRAFT_AUDIT_TOOL,
    NEUROSURGERY_PUBLIC_LITERATURE_MATRIX_TOOL,
    NEUROSURGERY_PUBLIC_LITERATURE_FRESHNESS_TOOL,
    NEUROSURGERY_PUBLIC_LITERATURE_REFRESH_AUDIT_TOOL,
    NEUROSURGERY_LITERATURE_LINK_AUDIT_TOOL,
    NEUROSURGERY_PUBLIC_LITERATURE_INTEGRITY_AUDIT_TOOL,
    NEUROSURGERY_PUBLIC_LITERATURE_REVIEW_QUEUE_TOOL,
    NEUROSURGERY_PUBLIC_LITERATURE_WORKBENCH_TOOL,
    NEUROSURGERY_PUBLIC_LITERATURE_PORTFOLIO_TOOL,
    NEUROSURGERY_RESEARCH_PLAN_TOOL,
    NEUROSURGERY_EVIDENCE_ACQUISITION_TOOL,
    NEUROSURGERY_RESEARCH_BRIEF_TOOL,
    NEUROSURGERY_TOOL,
    NEUROSURGERY_REAL_DATA_QUERY_TOOL,
    NEUROSURGERY_REAL_DATA_TRIAL_LANDSCAPE_TOOL,
    NEUROSURGERY_REAL_DATA_MOLECULAR_COVERAGE_TOOL,
    NEUROSURGERY_PUBLIC_LITERATURE_QUERY_TOOL,
    NEUROSURGERY_SESSION_TOOL,
    NEUROSURGERY_MISSION_TOOL,
  ].map((name) => ({ name, description: "guarded neurosurgical research tool", inputSchema: { type: "object" } }));
  const client = {
    calls,
    async tools() {
      return [...definitions, { name: "unrelated_tool", description: "other", inputSchema: { type: "object" } }];
    },
    async callTool(name, args = {}) {
      calls.push({ name, args });
      if (name === NEUROSURGERY_CATALOGUE_TOOL) return response(name, { schema_version: "catalogue", specialties: ["glioma"] });
      if (name === NEUROSURGERY_INTAKE_PLAN_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-intake-plan/0.1",
        plan_digest: "i".repeat(64),
        question_digest: "q".repeat(64),
        candidates: [{ specialty: "glioma", score_bps: 1000, matched_terms: ["glioma"] }],
        selected_specialty: "glioma",
        confidence_bps: 1000,
        abstained: false,
        reason: "selected",
        route: ["safety_gate", "glioma_molecular_panel", "human_review_hold"],
        evidence_sources: ["real_glioma_snapshot", "pubmed_snapshot"],
        reviewer_roles: ["neuro-oncology", "neurosurgery"],
        next_actions: ["Construct a CaseRequest with an explicit research_synthesis purpose."],
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: ["routing is lexical vocabulary matching"],
      });
      if (name === NEUROSURGERY_INTAKE_MISSION_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-intake-mission/0.1",
        intake: {
          schema_version: "bioprism-neurosurgery-intake-plan/0.1",
          plan_digest: "i".repeat(64),
          question_digest: "q".repeat(64),
          candidates: [],
          selected_specialty: "glioma",
          confidence_bps: 1000,
          abstained: false,
          reason: "selected",
          route: [],
          evidence_sources: ["real_glioma_snapshot"],
          reviewer_roles: [],
          next_actions: [],
          human_review_required: true,
          provider: "none",
          network: false,
          effect: "read_only",
          limitations: [],
        },
        status: "ready_for_human_review",
        request_digest: "r".repeat(64),
        mission: { status: "ready_for_human_review", provider: "none", network: false },
        required_evidence: [],
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: [],
      });
      if (name === NEUROSURGERY_INTAKE_PORTFOLIO_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-intake-portfolio/0.1",
        intake: { schema_version: "bioprism-neurosurgery-intake-plan/0.1", question_digest: "q".repeat(64), abstained: false },
        status: "ready_for_human_review",
        request_digest: "r".repeat(64),
        mission: null,
        portfolio: { specialty_count: 6, provider: "none", network: false, synthetic_data: false },
        selected_specialties: ["glioma", "cranial_base", "craniosynostosis", "encephalocele", "spina_bifida", "chiari_malformation"],
        required_evidence: [],
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: [],
      });
      if (name === NEUROSURGERY_EVIDENCE_AUDIT_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-evidence-audit/0.1",
        request_digest: "a".repeat(64),
        specialty: "encephalocele",
        required_observation_kinds: ["imaging"],
        items: [],
        missing_required_kinds: ["imaging"],
        coverage_complete: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        temporal_alignment: {
          schema_version: "bioprism-neurosurgery-temporal-alignment/0.1",
          status: "unavailable",
          coverage_complete: false,
          provider: "none",
          network: false,
          effect: "read_only",
        },
      });
      if (name === NEUROSURGERY_SPECIALTY_EVIDENCE_MAP_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-specialty-evidence-map/0.1",
        map_digest: "m".repeat(64),
        request_digest: "r".repeat(64),
        specialty: "glioma",
        dimensions: [{
          key: "tumor_identity",
          label: "Tumor identity and assay scope",
          required_observation_kinds: ["histology", "molecular"],
          required_kind_count: 2,
          covered_kind_count: 0,
          observed_observation_count: 0,
          not_collected_observation_count: 0,
          uninterpretable_observation_count: 0,
          conflicting_observation_count: 0,
          missing_provenance_count: 0,
          timestamped_observation_count: 0,
          timepoint_count: 0,
          source_ids: [],
          state: "not_collected",
          reviewer_question: "Which identity inputs are directly measured?",
        }],
        required_dimension_count: 1,
        complete_dimension_count: 0,
        partial_dimension_count: 0,
        not_collected_dimension_count: 1,
        uninterpretable_dimension_count: 0,
        conflicting_dimension_count: 0,
        observed_observation_count: 0,
        evidence_record_count: 0,
        verified_evidence_record_count: 0,
        missing_provenance_count: 0,
        timestamped_observation_count: 0,
        reviewer_questions: ["Which identity inputs are directly measured?"],
        state: "not_collected",
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: [],
      });
      if (name === NEUROSURGERY_CASE_ASSET_MANIFEST_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-case-asset-manifest/0.1",
        request_digest: "r".repeat(64),
        manifest_digest: "m".repeat(64),
        report_digest: "d".repeat(64),
        specialty: "glioma",
        asset_count: 1,
        observed_asset_count: 1,
        non_observed_asset_count: 0,
        provenance_complete_asset_count: 1,
        coverage: [],
        requested_kinds: ["imaging_series"],
        missing_requested_kinds: [],
        assets: [],
        review_items: [],
        omitted_review_item_count: 0,
        truncated: false,
        deidentified: true,
        raw_values_retained: false,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: [],
      });
      if (name === NEUROSURGERY_CASE_FHIR_IMPORT_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-case-fhir-import/0.1",
        request_digest: "r".repeat(64),
        bundle_digest: "b".repeat(64),
        hints_digest: "h".repeat(64),
        report_digest: "d".repeat(64),
        specialty: "glioma",
        resource_count: 1,
        projected_asset_count: 1,
        unclassified_resource_count: 0,
        manifest_report: {},
        review_items: [],
        omitted_review_item_count: 0,
        truncated: false,
        deidentified: true,
        raw_values_retained: false,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: [],
      });
      if (name === NEUROSURGERY_CASE_DICOM_IMPORT_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-case-dicom-import/0.1",
        request_digest: "r".repeat(64),
        datasets_digest: "b".repeat(64),
        report_digest: "d".repeat(64),
        specialty: "glioma",
        dataset_count: 1,
        projected_series_count: 1,
        unclassified_dataset_count: 0,
        series: [],
        manifest_report: {},
        review_items: [],
        omitted_review_item_count: 0,
        truncated: false,
        deidentified: true,
        raw_values_retained: false,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: [],
      });
      if (name === NEUROSURGERY_CASE_DICOM_EVIDENCE_WORKFLOW_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-case-dicom-evidence-workflow/0.1",
        workflow_digest: "w".repeat(64),
        request_digest: "r".repeat(64),
        specialty: "glioma",
        query: args.query ?? {},
        dicom_import: { schema_version: "bioprism-neurosurgery-case-dicom-import/0.1" },
        evidence_synthesis: {},
        evidence_program: {},
        evidence_acquisition: {},
        evidence_acquisition_session: {},
        status: "ready_for_human_review",
        human_review_required: true,
        provenance_bound: true,
        synthetic_data: false,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: [],
      });
      if (name === NEUROSURGERY_CASE_ASSET_REVIEW_DISPOSITION_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-case-asset-review-disposition/0.1",
        report_digest: "d".repeat(64),
        disposition_digest: "x".repeat(64),
        candidate_item_count: 2,
        returned_item_count: 2,
        omitted_item_count: 0,
        submitted_decision_count: args.decisions?.length ?? 0,
        accepted_decision_count: args.decisions?.length ?? 0,
        resolved_decision_count: args.decisions?.length ?? 0,
        unresolved_decision_count: 0,
        undecided_returned_item_count: 0,
        pending_item_count: 0,
        decisions: args.decisions ?? [],
        unresolved_sequences: [],
        undecided_sequences: [],
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: [],
      });
      if (name === NEUROSURGERY_EVIDENCE_PROGRAM_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-evidence-program/0.1",
        program_digest: "e".repeat(64),
        request_digest: "r".repeat(64),
        generated_at: "2026-08-30T00:00:00Z",
        query: args.query ?? {},
        lanes: [{
          specialty: "glioma",
          tracks: [{
            track_id: "imaging_phenotype",
            label: "Imaging phenotype",
            review_objective: "review",
            search_terms: ["MRI"],
            required_observation_kinds: ["imaging"],
            observation_coverage: [],
            missing_observation_kinds: ["imaging"],
            observation_coverage_complete: false,
            observation_provenance_complete: false,
            asset_coverage: [],
            missing_asset_kinds: ["imaging_series"],
            asset_coverage_complete: false,
            review_worklist: [{ code: "asset_class_missing", asset_kind: "imaging_series", detail: "review" }],
            reviewer_roles: ["neurosurgeon"],
            real_match_count: 0,
            real_returned_count: 0,
            real_truncated: false,
            public_match_count: 0,
            public_returned_count: 0,
            public_truncated: false,
            references: [],
            reference_omitted_count: 0,
            human_review_required: true,
          }],
          track_count: 1,
          non_empty_track_count: 0,
          empty_track_ids: ["imaging_phenotype"],
        }],
        specialty_count: 1,
        non_empty_lane_count: 0,
        empty_lane_specialties: ["glioma"],
        total_track_count: 1,
        non_empty_track_count: 0,
        reference_count: 0,
        reference_omitted_count: 0,
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: [],
      });
      if (name === NEUROSURGERY_EVIDENCE_SYNTHESIS_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-evidence-synthesis/0.1",
        synthesis_digest: "s".repeat(64),
        request_digest: "r".repeat(64),
        specialty: "glioma",
        generated_at: "2026-08-30T00:00:00Z",
        query: args.query,
        case_observations: [],
        case_audit: { schema_version: "bioprism-neurosurgery-evidence-audit/0.1" },
        references: [{ plane: "public_literature", record_id: "PMID-12345678" }],
        lanes: [],
        real_data_summary: null,
        public_literature_summary: { bundle_digest: "f".repeat(64) },
        literature_link_audit: null,
        links: [],
        review_items: [],
        reviewer_roles: ["neurosurgery"],
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: ["alignment only"],
      });
      if (name === NEUROSURGERY_EVIDENCE_GRAPH_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-evidence-graph/0.1",
        bundle_digest: "b".repeat(64),
        graph_digest: "g".repeat(64),
        specialty: "glioma",
        query: args.query,
        nodes: [],
        edges: [],
        total_node_count: 0,
        total_edge_count: 0,
        omitted_node_count: 0,
        omitted_edge_count: 0,
        truncated: false,
        root_count: 0,
        connected_component_count: 0,
        isolated_node_count: 0,
        source_count: 0,
        bundle_relationship_count: 0,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: ["explicit_crosswalk_only"],
      });
      if (name === NEUROSURGERY_GLIOMA_MOLECULAR_MAP_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-glioma-molecular-map/0.1",
        map_digest: "m".repeat(64),
        request_digest: "r".repeat(64),
        specialty: "glioma",
        generated_at: "2026-08-30T00:00:00Z",
        query: args.query,
        panel: null,
        real_data_digest: "b".repeat(64),
        public_literature_digest: "p".repeat(64),
        markers: [],
        references: [],
        review_items: [],
        reviewer_roles: ["neuro-oncology"],
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: ["retrieval metadata only"],
      });
      if (name === NEUROSURGERY_REAL_DATA_COVERAGE_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-real-data-coverage/0.1",
        bundle_digest: "b".repeat(64),
        coverage_digest: "c".repeat(64),
        generated_at: "2026-08-30T00:00:00Z",
        query: args.query,
        total_record_count: 88,
        matched_record_count: 4,
        source_count: 5,
        sources: [{ source_id: "fixture", kind: "literature_index", authority: "fixture authority", uri: "https://example.test/fixture", retrieved_at: "2026-08-30T00:00:00Z", declared_record_count: 4, observed_record_count: 4, selected_record_count: 4 }],
        record_kind_counts: [{ record_kind: "literature_article", count: 4 }],
        time_axes: [{ axis: "literature_publication_date", observed_count: 4, missing_count: 0, earliest: "2026-01-01", latest: "2026-08-30", year_buckets: [{ year: 2026, count: 4 }] }],
        portal_profile_type_counts: [{ alteration_type: "MUTATION", count: 1 }],
        linkage: { portal_study_count: 0, portal_study_with_pmid_count: 0, portal_study_without_pmid_count: 0, portal_molecular_profile_count: 0, explicit_profile_relationship_count: 0, literature_article_count: 4, literature_linked_to_portal_count: 0, literature_without_portal_count: 4, explicit_publication_relationship_count: 0, literature_abstract_count: 4, literature_abstract_missing_count: 0, literature_abstract_truncated_count: 0 },
        gaps: [{ code: "fixture_gap", count: 1, description: "fixture review gap" }],
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: ["fixture metadata only"],
      });
      if (name === NEUROSURGERY_REAL_DATA_COHORT_LANDSCAPE_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-real-data-cohort-landscape/0.1",
        landscape_digest: "l".repeat(64),
        bundle_digest: "b".repeat(64),
        generated_at: "2026-08-30T00:00:00Z",
        query: args.query,
        total_matching_projects: 2,
        returned_project_count: 2,
        omitted_project_count: 0,
        truncated: false,
        project_rows: [],
        total_released_case_inventory: 1133,
        data_type_coverage: [],
        shared_data_type_count: 0,
        shared_data_types: [],
        projects_with_data_type_metadata: 2,
        projects_without_data_type_metadata: 0,
        source_ids: ["gdc_tcga_gbm", "gdc_tcga_lgg"],
        review_reasons: [],
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: ["aggregate metadata only"],
      });
      if (name === NEUROSURGERY_REAL_DATA_RECONCILIATION_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-real-data-reconciliation/0.1",
        reconciliation_digest: "r".repeat(64),
        bundle_digest: "b".repeat(64),
        generated_at: "2026-08-30T00:00:00Z",
        query: args.query,
        counts: {
          portal_study_count: 7,
          portal_study_with_pmid_count: 6,
          portal_study_without_pmid_count: 1,
          portal_pmid_missing_literature_count: 0,
          shared_portal_pmid_count: 0,
          literature_article_count: 20,
          literature_with_doi_count: 20,
          shared_literature_doi_count: 0,
        },
        candidate_issue_count: 0,
        returned_issue_count: 0,
        omitted_issue_count: 0,
        truncated: false,
        issues: [],
        requires_review: false,
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: ["identifier reconciliation is metadata-only"],
      });
      if (name === NEUROSURGERY_REAL_DATA_FRESHNESS_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-real-data-freshness/0.1",
        bundle_digest: "b".repeat(64),
        generated_at: "2026-08-30T00:00:00Z",
        query: args.query,
        status: "stale",
        source_count: 5,
        current_source_count: 0,
        stale_source_count: 5,
        future_dated_source_count: 0,
        sources: [],
        freshness_digest: "f".repeat(64),
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: [],
      });
      if (name === NEUROSURGERY_REAL_DATA_DIFF_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-real-data-diff/0.1",
        before_bundle_digest: "b".repeat(64),
        after_bundle_digest: "a".repeat(64),
        diff_digest: "d".repeat(64),
        before_generated_at: "2026-08-30T00:00:00Z",
        after_generated_at: "2026-08-31T00:00:00Z",
        query: args.query,
        before_record_count: 88,
        after_record_count: 88,
        record_counts: { added: 0, removed: 0, changed: 1 },
        source_counts: { added: 0, removed: 0, changed: 1 },
        total_change_count: 2,
        returned_change_count: 2,
        omitted_record_change_count: 0,
        omitted_source_change_count: 0,
        truncated: false,
        record_changes: [],
        source_changes: [],
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: [],
      });
      if (name === NEUROSURGERY_REAL_DATA_REFRESH_AUDIT_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-real-data-refresh-audit/0.1",
        audit_digest: "r".repeat(64),
        before_bundle_digest: "b".repeat(64),
        after_bundle_digest: "a".repeat(64),
        before_generated_at: "2026-08-30T00:00:00Z",
        after_generated_at: "2026-08-31T00:00:00Z",
        query: args.query ?? {},
        diff: { schema_version: "bioprism-neurosurgery-real-data-diff/0.1" },
        coverage: { schema_version: "bioprism-neurosurgery-real-data-coverage/0.1" },
        freshness: null,
        review_queue: { schema_version: "bioprism-neurosurgery-real-data-review-queue/0.1" },
        research_brief: { schema_version: "bioprism-neurosurgery-real-data-research-brief/0.1", source: "real_glioma" },
        structural_change_detected: false,
        source_identity_stable: true,
        record_identity_stable: true,
        requires_refresh_review: true,
        review_reasons: [{ code: "metadata_obligations", count: 1, detail: "verify metadata" }],
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: ["candidate snapshot is never accepted"],
      });
      if (name === NEUROSURGERY_REAL_DATA_REVIEW_QUEUE_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-real-data-review-queue/0.1",
        bundle_digest: "b".repeat(64),
        queue_digest: "q".repeat(64),
        generated_at: "2026-08-30T00:00:00Z",
        query: args.query,
        source_count: 5,
        record_count: 88,
        candidate_item_count: 15,
        returned_item_count: 2,
        omitted_item_count: 13,
        truncated: true,
        items: [],
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: [],
      });
      if (name === NEUROSURGERY_REAL_DATA_REVIEW_DISPOSITION_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-real-data-review-disposition/0.1",
        bundle_digest: "b".repeat(64),
        queue_digest: "q".repeat(64),
        disposition_digest: "d".repeat(64),
        candidate_item_count: 15,
        queue_returned_item_count: 2,
        queue_omitted_item_count: 13,
        submitted_decision_count: args.decisions?.length ?? 0,
        accepted_decision_count: args.decisions?.length ?? 0,
        resolved_decision_count: args.decisions?.length ?? 0,
        unresolved_decision_count: 0,
        undecided_returned_item_count: 2 - (args.decisions?.length ?? 0),
        pending_item_count: 13,
        decisions: args.decisions ?? [],
        unresolved_task_ids: [],
        undecided_task_ids: [],
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: [],
      });
      if (name === NEUROSURGERY_REAL_DATA_EVIDENCE_PACKET_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-real-data-evidence-packet/0.4",
        packet_digest: "p".repeat(64),
        bundle_digest: "b".repeat(64),
        generated_at: "2026-08-30T00:00:00Z",
        query: args.query,
        summary: { bundle_digest: "b".repeat(64) },
        coverage: { coverage_digest: "c".repeat(64) },
        graph: { graph_digest: "g".repeat(64) },
        data_query: { total_matches: 4, query: args.query?.query ?? {} },
        trial_landscape: {
          schema_version: "bioprism-neurosurgery-real-data-trial-landscape/0.1",
          landscape_digest: "l".repeat(64),
          bundle_digest: "b".repeat(64),
          generated_at: "2026-08-30T00:00:00Z",
          query: { query: {}, max_interventions: 128 },
          total_matching_trials: 5,
          returned_trial_count: 5,
          omitted_trial_count: 0,
          truncated: false,
          status_counts: [],
          phase_counts: [],
          phase_annotated_trial_count: 5,
          study_type_counts: [],
          intervention_counts: [],
          distinct_intervention_count: 0,
          omitted_intervention_count: 0,
          intervention_truncated: false,
          missing_phase_count: 0,
          missing_last_update_count: 0,
          missing_study_type_count: 0,
          missing_enrollment_count: 0,
          missing_intervention_count: 0,
          earliest_last_update: null,
          latest_last_update: null,
          source_ids: [],
          review_reasons: [],
          provenance_bound: true,
          synthetic_data: false,
          human_review_required: true,
          provider: "none",
          network: false,
          effect: "read_only",
          limitations: ["metadata only"],
        },
        molecular_coverage: {
          schema_version: "bioprism-neurosurgery-real-data-molecular-coverage/0.1",
          coverage_digest: "m".repeat(64),
          bundle_digest: "b".repeat(64),
          generated_at: "2026-08-30T00:00:00Z",
          query: { query: { limit: 128 }, max_studies: 128 },
          total_matching_profile_count: 54,
          returned_profile_count: 54,
          omitted_profile_count: 0,
          truncated: false,
          distinct_returned_study_count: 7,
          emitted_study_count: 7,
          omitted_study_count: 0,
          study_rows_truncated: false,
          emitted_profile_count: 54,
          study_rows: [],
          alteration_type_counts: [],
          datatype_counts: [],
          patient_level_profile_count: 0,
          analysis_visible_profile_count: 0,
          description_present_count: 54,
          missing_description_count: 0,
          missing_alteration_type_count: 0,
          missing_datatype_count: 0,
          missing_study_link_count: 0,
          source_ids: [],
          review_reasons: [],
          provenance_bound: true,
          synthetic_data: false,
          human_review_required: true,
          provider: "none",
          network: false,
          effect: "read_only",
          limitations: ["metadata only"],
        },
        cohort_landscape: {
          schema_version: "bioprism-neurosurgery-real-data-cohort-landscape/0.1",
          landscape_digest: "h".repeat(64),
          bundle_digest: "b".repeat(64),
          generated_at: "2026-08-30T00:00:00Z",
          query: { query: { record_kind: "genomic_project", limit: 32 }, max_projects: 32 },
          total_matching_projects: 1,
          returned_project_count: 1,
          omitted_project_count: 0,
          truncated: false,
          project_rows: [{
            project_id: "TCGA-GBM",
            source_id: "gdc",
            source_uri: "https://portal.gdc.cancer.gov/projects/TCGA-GBM",
            name: "TCGA Glioblastoma",
            primary_site: ["Brain"],
            disease_types: ["Glioblastoma"],
            case_count: 617,
            data_type_metadata_present: true,
            data_type_counts: [],
            total_file_count: 4822,
          }],
          total_released_case_inventory: 617,
          data_type_coverage: [],
          shared_data_type_count: 0,
          shared_data_types: [],
          projects_with_data_type_metadata: 1,
          projects_without_data_type_metadata: 0,
          source_ids: ["gdc"],
          review_reasons: [],
          provenance_bound: true,
          synthetic_data: false,
          human_review_required: true,
          provider: "none",
          network: false,
          effect: "read_only",
          limitations: ["metadata only"],
        },
        review_queue: { candidate_item_count: 15 },
        source_count: 5,
        record_count: 88,
        query_match_count: 4,
        open_review_obligation_count: 15,
        explicit_crosswalk_edge_count: 60,
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: [],
      });
      if (name === NEUROSURGERY_REAL_DATA_AUTONOMOUS_WORKFLOW_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-real-data-autonomous-workflow/0.1",
        workflow_digest: "w".repeat(64),
        bundle_digest: "b".repeat(64),
        packet_digest: "p".repeat(64),
        generated_at: "2026-08-30T00:00:00Z",
        query: args.query,
        packet: { packet_digest: "p".repeat(64), bundle_digest: "b".repeat(64) },
        state: "needs_metadata_review",
        candidate_action_count: 1,
        returned_action_count: 1,
        omitted_action_count: 0,
        truncated: false,
        resolved_queue_item_count: 0,
        open_queue_item_count: 1,
        actions: [],
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: [],
      });
      if (name === NEUROSURGERY_REAL_DATA_REASONING_CONTEXT_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-real-data-reasoning-context/0.1",
        context_digest: "c".repeat(64),
        packet_digest: "p".repeat(64),
        bundle_digest: "b".repeat(64),
        generated_at: "2026-08-30T00:00:00Z",
        query: args.query,
        context_text: "# AURORA REAL-GLIOMA REASONING CONTEXT",
        citations: [
          {
            record_kind: "genomic_project",
            record_id: "TCGA-GBM",
            title: "TCGA-GBM",
            source_id: "gdc_tcga",
            source_uri: "https://portal.gdc.cancer.gov/projects/TCGA-GBM",
            abstract_included: false,
          },
          {
            record_kind: "clinical_trial",
            record_id: "NCT00000001",
            title: "A bounded clinical trial",
            source_id: "clinicaltrials_glioma",
            source_uri: "https://clinicaltrials.gov/study/NCT00000001",
            abstract_included: false,
          },
          {
            record_kind: "portal_molecular_profile",
            record_id: "profile-1",
            title: "A bounded molecular profile",
            source_id: "cbioportal_glioma",
            source_uri: "https://www.cbioportal.org/study/gbm_tcga",
            abstract_included: false,
          },
        ],
        included_citation_count: 3,
        omitted_citation_count: 0,
        context_char_count: 38,
        truncated: false,
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: [],
      });
      if (name === NEUROSURGERY_RESEARCH_BRIEF_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-research-brief/0.1",
        brief_digest: "q".repeat(64), request_digest: "x".repeat(64), source: "real_glioma", specialty: "glioma",
        bundle_digest: "b".repeat(64), generated_at: "2026-08-31T00:00:00Z", query: args.query,
        topic_count: 1, non_empty_topic_count: 1, total_match_count: 1, total_returned_count: 1,
        cross_topic_record_count: 0, source_query_truncated: false,
        topics: [{ topic_id: "molecular_identity", label: "Integrated molecular identity", terms: ["idh", "mgmt"],
          matched_record_count: 1, returned_record_count: 1, truncated: false, source_ids: ["pubmed_glioma_molecular"],
          publication_type_counts: [{ label: "Review", count: 1 }], abstract_count: 0,
          records: [{ record_kind: "literature_article", record_id: "12345678", title: "IDH and MGMT in diffuse glioma",
            source_id: "pubmed_glioma_molecular", source_uri: "https://pubmed.ncbi.nlm.nih.gov/",
            record_uri: "https://pubmed.ncbi.nlm.nih.gov/12345678/", publication_date: "2024-01-01", matched_terms: ["idh", "mgmt"], publication_types: ["Review"], mesh_terms: ["Glioma"],
            abstract_excerpt: "must not leak" }],
        }],
        unknowns: [{ code: "topic_unknown", scope: "molecular_identity", detail: "reviewer check" }], review_prompts: ["Confirm lexical topic membership."], limitations: ["metadata only"],
        provenance_bound: true, synthetic_data: false, human_review_required: true, provider: "none", network: false, effect: "read_only",
      });
      if (name === NEUROSURGERY_REAL_DATA_DRAFT_AUDIT_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-real-data-draft-audit/0.1",
        draft_digest: "d".repeat(64),
        packet_digest: "p".repeat(64),
        bundle_digest: "b".repeat(64),
        generated_at: "2026-08-30T00:00:00Z",
        packet: { packet_digest: "p".repeat(64) },
        claims: [],
        claim_count: args.claims?.length ?? 0,
        grounded_claim_count: args.claims?.length ?? 0,
        blocked_claim_count: 0,
        status: "grounded_for_human_review",
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: [],
      });
      if (name === NEUROSURGERY_PUBLIC_LITERATURE_EVIDENCE_PACKET_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-public-literature-evidence-packet/0.1",
        packet_digest: "p".repeat(64),
        bundle_digest: "f".repeat(64),
        generated_at: "2026-08-30T00:00:00Z",
        query: args.query,
        summary: { bundle_digest: "f".repeat(64) },
        query_result: { total_matches: 1, returned_matches: 1, hits: [{ pmid: "12345678" }] },
        source_count: 1,
        record_count: 145,
        query_match_count: 1,
        abstract_count: 138,
        abstract_truncated_count: 0,
        specialty_counts: [],
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: [],
      });
      if (name === NEUROSURGERY_PUBLIC_LITERATURE_REASONING_CONTEXT_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-public-literature-reasoning-context/0.1",
        context_digest: "c".repeat(64),
        packet_digest: "p".repeat(64),
        bundle_digest: "f".repeat(64),
        generated_at: "2026-08-30T00:00:00Z",
        query: args.query,
        context_text: "# AURORA PUBLIC-NEUROSURGICAL LITERATURE REASONING CONTEXT",
        citations: [{
          specialty: "chiari_malformation",
          pmid: "12345678",
          title: "A bounded Chiari citation",
          source_id: "pubmed_chiari",
          source_uri: "https://pubmed.ncbi.nlm.nih.gov/12345678/",
          record_uri: "https://pubmed.ncbi.nlm.nih.gov/12345678/",
          abstract_included: false,
        }],
        included_citation_count: 1,
        omitted_citation_count: 0,
        context_char_count: 60,
        truncated: false,
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: [],
      });
      if (name === NEUROSURGERY_PUBLIC_LITERATURE_DRAFT_AUDIT_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-public-literature-draft-audit/0.1",
        draft_digest: "d".repeat(64),
        packet_digest: "p".repeat(64),
        bundle_digest: "f".repeat(64),
        generated_at: "2026-08-30T00:00:00Z",
        packet: { packet_digest: "p".repeat(64) },
        claims: [],
        claim_count: args.claims?.length ?? 0,
        grounded_claim_count: args.claims?.length ?? 0,
        blocked_claim_count: 0,
        status: "grounded_for_human_review",
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: [],
      });
      if (name === NEUROSURGERY_PUBLIC_LITERATURE_MATRIX_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-public-literature-matrix/0.1",
        matrix_digest: "m".repeat(64),
        bundle_digest: "f".repeat(64),
        generated_at: "2026-08-30T00:00:00Z",
        query: args.query,
        lanes: [
          { specialty: "glioma", packet: { packet_digest: "p".repeat(64) } },
          { specialty: "chiari_malformation", packet: { packet_digest: "p".repeat(64) } },
        ],
        specialty_count: 2,
        non_empty_lane_count: 2,
        empty_lane_specialties: [],
        total_match_count: 2,
        total_returned_count: 2,
        truncated_lane_count: 0,
        returned_abstract_count: 2,
        returned_without_abstract_count: 0,
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: [],
      });
      if (name === NEUROSURGERY_PUBLIC_LITERATURE_FRESHNESS_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-real-data-freshness/0.1",
        bundle_digest: "f".repeat(64),
        generated_at: "2026-08-30T00:00:00Z",
        query: args.query,
        status: "current",
        source_count: 1,
        current_source_count: 1,
        stale_source_count: 0,
        future_dated_source_count: 0,
        sources: [],
        freshness_digest: "f".repeat(64),
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: [],
      });
      if (name === NEUROSURGERY_PUBLIC_LITERATURE_REFRESH_AUDIT_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-public-literature-refresh-audit/0.1",
        audit_digest: "r".repeat(64),
        before_bundle_digest: "b".repeat(64),
        after_bundle_digest: "a".repeat(64),
        before_generated_at: "2026-08-30T00:00:00Z",
        after_generated_at: "2026-08-31T00:00:00Z",
        query: args.query,
        before_summary: {},
        after_summary: {},
        diff: { schema_version: "bioprism-neurosurgery-public-literature-refresh-diff/0.1" },
        matrix: { schema_version: "bioprism-neurosurgery-public-literature-matrix/0.1" },
        freshness: null,
        structural_change_detected: false,
        specialty_coverage_changed: false,
        source_identity_stable: true,
        record_identity_stable: true,
        requires_refresh_review: false,
        review_reasons: [],
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: [],
      });
      if (name === NEUROSURGERY_LITERATURE_LINK_AUDIT_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-literature-link-audit/0.1",
        audit_digest: "l".repeat(64),
        real_data_bundle_digest: "r".repeat(64),
        public_literature_bundle_digest: "p".repeat(64),
        real_data_generated_at: "2026-08-30T00:00:00Z",
        public_literature_generated_at: "2026-08-30T00:00:00Z",
        query: args.query,
        real_data_summary: {},
        public_literature_summary: {},
        counts: {
          real_literature_records: 20,
          selected_public_literature_records: 25,
          linked_real_records: 12,
          linked_public_records: 12,
          unmatched_real_records: 8,
          unmatched_public_records: 13,
          pmid_match_count: 12,
          doi_match_count: 12,
          metadata_mismatch_count: 0,
          identifier_conflict_count: 0,
        },
        links: [],
        unmatched_real_pmids: [],
        unmatched_public_pmids: [],
        omitted_link_count: 0,
        omitted_unmatched_real_count: 0,
        omitted_unmatched_public_count: 0,
        truncated: false,
        requires_link_review: true,
        review_reasons: [],
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: [],
      });
      if (name === NEUROSURGERY_PUBLIC_LITERATURE_INTEGRITY_AUDIT_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-public-literature-integrity-audit/0.1",
        audit_digest: "i".repeat(64),
        bundle_digest: "p".repeat(64),
        generated_at: "2026-08-30T00:00:00Z",
        query: args.query,
        summary: {},
        counts: {
          selected_record_count: 145,
          selected_source_count: 6,
          unique_pmid_count: 145,
          doi_count: 145,
          missing_doi_count: 0,
          abstract_count: 138,
          missing_abstract_count: 7,
          abstract_truncated_count: 0,
          empty_publication_type_count: 0,
          empty_mesh_term_count: 84,
          duplicate_doi_group_count: 0,
          cross_specialty_duplicate_doi_group_count: 0,
        },
        issues: [{ code: "missing_abstract", specialty: "glioma", pmid: "PMID-12345678", source_id: "pubmed_glioma", related_pmids: [], detail: "abstract metadata is absent" }],
        omitted_issue_count: 0,
        truncated: false,
        requires_integrity_review: true,
        review_reasons: [],
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: [],
      });
      if (name === NEUROSURGERY_PUBLIC_LITERATURE_REVIEW_QUEUE_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-public-literature-review-queue/0.1",
        bundle_digest: "p".repeat(64),
        queue_digest: "q".repeat(64),
        integrity_audit_digest: "i".repeat(64),
        generated_at: "2026-08-30T00:00:00Z",
        query: args.query,
        candidate_item_count: 3,
        returned_item_count: 3,
        omitted_item_count: 0,
        omitted_integrity_issue_count: 0,
        truncated: false,
        items: [],
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: [],
      });
      if (name === NEUROSURGERY_PUBLIC_LITERATURE_WORKBENCH_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-public-literature-workbench/0.1",
        workbench_digest: "w".repeat(64),
        bundle_digest: "p".repeat(64),
        generated_at: "2026-08-30T00:00:00Z",
        query: args.query,
        lanes: [
          {
            specialty: "glioma",
            profile: {
              specialty: "glioma",
              identity_axes: ["molecular"],
              spatial_axes: ["anatomic"],
              temporal_axes: ["longitudinal"],
              evidence_questions: ["what is observed?"],
              confounders: ["sampling"],
              human_review_roles: ["neuro-oncology"]
            },
            source_ids: ["pubmed_glioma"],
            record_count: 25,
            abstract_count: 25,
            abstract_truncated_count: 0,
            missing_doi_count: 0,
            missing_abstract_count: 0,
            empty_publication_type_count: 0,
            empty_mesh_term_count: 3,
            review_issue_count: 3,
            omitted_review_issue_count: 0,
            truncated: false,
            integrity_audit_digest: "i".repeat(64),
            review_reasons: [],
          },
        ],
        specialty_count: 1,
        non_empty_lane_count: 1,
        empty_lane_specialties: [],
        total_record_count: 25,
        total_review_issue_count: 3,
        omitted_review_issue_count: 0,
        truncated_lane_count: 0,
        freshness: null,
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: [],
      });
      if (name === NEUROSURGERY_PUBLIC_LITERATURE_PORTFOLIO_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-public-literature-portfolio/0.1",
        portfolio_digest: "o".repeat(64),
        bundle_digest: "p".repeat(64),
        generated_at: "2026-08-30T00:00:00Z",
        query: args.query,
        lanes: [{
          specialty: "glioma",
          workbench: { specialty: "glioma", record_count: 25 },
          query_result: { total_matches: 25, returned_matches: 2, truncated: true, hits: [], abstract_count: 0, abstract_truncated_count: 0, specialty_counts: [] },
          review_queue: { candidate_item_count: 3, returned_item_count: 2, omitted_item_count: 1, truncated: true, items: [] },
        }],
        specialty_count: 1,
        non_empty_lane_count: 1,
        empty_lane_specialties: [],
        total_match_count: 25,
        total_returned_count: 2,
        total_review_issue_count: 3,
        total_review_item_count: 3,
        omitted_review_item_count: 1,
        truncated_lane_count: 1,
        freshness: null,
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: [],
      });
      if (name === NEUROSURGERY_RESEARCH_PLAN_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-research-plan/0.1",
        request_digest: "p".repeat(64),
        specialty: args.request?.specialty ?? "encephalocele",
        audit: {
          schema_version: "bioprism-neurosurgery-evidence-audit/0.1",
          request_digest: "a".repeat(64),
          specialty: "encephalocele",
          required_observation_kinds: [],
          items: [],
          missing_required_kinds: [],
          coverage_complete: true,
          human_review_required: true,
          provider: "none",
          network: false,
          effect: "read_only",
        },
        tasks: [],
        candidate_task_count: 0,
        omitted_task_count: 0,
        truncated: false,
        source_query_count: 0,
        source_candidate_count: 0,
        coverage_complete: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: ["caller_observations_required"],
      });
      if (name === NEUROSURGERY_EVIDENCE_ACQUISITION_TOOL) {
        const operation = args.operation ?? "compile";
        if (operation === "start") return response(name, {
          schema_version: "bioprism-neurosurgery-evidence-acquisition-session/0.1",
          plan: { schema_version: "bioprism-neurosurgery-evidence-acquisition/0.1" },
          session: {
            schema_version: "bioprism-neurosurgery-evidence-acquisition-session/0.1",
            session_id: `nsa-session-${"e".repeat(16)}`,
            plan_digest: "e".repeat(64),
            request_digest: "p".repeat(64),
            specialty: args.request?.specialty ?? "glioma",
            next_sequence: 1,
            status: "planned",
            event_chain_digest: "c".repeat(64),
            events: [],
          },
          human_review_required: true,
          provider: "none",
          network: false,
          effect: "read_only",
        });
        if (operation === "advance") return response(name, {
          schema_version: "bioprism-neurosurgery-evidence-acquisition-execution/0.1",
          session: { ...args.session, next_sequence: (args.session?.next_sequence ?? 1) + 1, status: "awaiting_human_review" },
          steps_executed: 1,
          complete: true,
          steps: [],
          human_review_required: true,
          provider: "none",
          network: false,
          effect: "read_only",
          limitations: [],
        });
        if (operation === "finish") return response(name, {
          schema_version: "bioprism-neurosurgery-evidence-acquisition-execution/0.1",
          plan_digest: "e".repeat(64),
          request_digest: "p".repeat(64),
          specialty: args.request?.specialty ?? "glioma",
          steps_executed: 1,
          event_count: 1,
          event_chain_digest: "c".repeat(64),
          human_review_required: true,
          provider: "none",
          network: false,
          effect: "read_only",
          limitations: [],
        });
        return response(name, {
          schema_version: "bioprism-neurosurgery-evidence-acquisition/0.1",
          plan_digest: "e".repeat(64),
          request_digest: "p".repeat(64),
          specialty: args.request?.specialty ?? "glioma",
          query: args.query ?? {},
          audit: {},
          steps: [],
          candidate_step_count: 0,
          omitted_step_count: 0,
          truncated: false,
          source_query_count: 0,
          source_candidate_count: 0,
          required_sources: [],
          ready_for_local_replay: false,
          human_review_required: true,
          provider: "none",
          network: false,
          effect: "read_only",
          limitations: [],
        });
      }
      if (name === NEUROSURGERY_RESEARCH_BRIEF_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-research-brief/0.1",
        brief_digest: "r".repeat(64),
        request_digest: "q".repeat(64),
        source: "real_glioma",
        specialty: args.request?.specialty ?? "glioma",
        bundle_digest: "b".repeat(64),
        generated_at: "2026-08-30T00:00:00Z",
        query: args.query ?? {},
        topics: [{ topic_id: "caller_focus", label: "caller focus terms", terms: ["glioblastoma"], matched_record_count: 1, returned_record_count: 1, truncated: false, source_ids: ["pubmed_glioblastoma"], publication_type_counts: [], abstract_count: 0, records: [] }],
        topic_count: 1,
        non_empty_topic_count: 1,
        total_match_count: 1,
        total_returned_count: 1,
        cross_topic_record_count: 0,
        source_query_truncated: false,
        unknowns: [],
        review_prompts: ["verify source scope"],
        freshness: null,
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: ["lexical extraction only"],
      });
      if (name === NEUROSURGERY_REAL_DATA_QUERY_TOOL) return response(name, {
        schema_version: "query",
        matched_records: [{ id: "trial-1" }],
        returned_count: 1,
        total_matches: 1,
        returned_matches: 1,
        truncated: false,
        hits: [{ record_kind: "clinical_trial", record_id: "TOOL-TRIAL", title: "Tool-discovered trial", source_id: "clinicaltrials_glioma", source_uri: "https://clinicaltrials.gov/study/TOOL-TRIAL", record_uri: "https://clinicaltrials.gov/study/TOOL-TRIAL" }],
      });
      if (name === NEUROSURGERY_REAL_DATA_TRIAL_LANDSCAPE_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-real-data-trial-landscape/0.1",
        landscape_digest: "l".repeat(64),
        bundle_digest: "b".repeat(64),
        generated_at: "2026-08-30T00:00:00Z",
        query: args.query,
        total_matching_trials: 2,
        returned_trial_count: 2,
        omitted_trial_count: 0,
        truncated: false,
        status_counts: [{ label: "RECRUITING", count: 2 }],
        phase_counts: [{ label: "PHASE2", count: 2 }],
        phase_annotated_trial_count: 2,
        study_type_counts: [{ label: "INTERVENTIONAL", count: 2 }],
        intervention_counts: [{ name: "temozolomide", count: 2 }],
        distinct_intervention_count: 1,
        omitted_intervention_count: 0,
        intervention_truncated: false,
        missing_phase_count: 0,
        missing_last_update_count: 0,
        missing_study_type_count: 0,
        missing_enrollment_count: 0,
        missing_intervention_count: 0,
        earliest_last_update: "2023-01-01",
        latest_last_update: "2024-12-31",
        source_ids: ["clinicaltrials_glioma"],
        review_reasons: [],
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: ["metadata only"],
      });
      if (name === NEUROSURGERY_REAL_DATA_MOLECULAR_COVERAGE_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-real-data-molecular-coverage/0.1",
        coverage_digest: "m".repeat(64),
        bundle_digest: "b".repeat(64),
        generated_at: "2026-08-30T00:00:00Z",
        query: args.query,
        total_matching_profile_count: 6,
        returned_profile_count: 6,
        omitted_profile_count: 0,
        truncated: false,
        distinct_returned_study_count: 6,
        emitted_study_count: 6,
        omitted_study_count: 0,
        study_rows_truncated: false,
        emitted_profile_count: 6,
        study_rows: [],
        alteration_type_counts: [{ label: "MUTATION_EXTENDED", count: 6 }],
        datatype_counts: [{ label: "MAF", count: 6 }],
        patient_level_profile_count: 0,
        analysis_visible_profile_count: 6,
        description_present_count: 6,
        missing_description_count: 0,
        missing_alteration_type_count: 0,
        missing_datatype_count: 0,
        missing_study_link_count: 0,
        source_ids: ["cbioportal_gbm"],
        review_reasons: [],
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none",
        network: false,
        effect: "read_only",
        limitations: ["metadata only"],
      });
      if (name === NEUROSURGERY_PUBLIC_LITERATURE_QUERY_TOOL) return response(name, {
        schema_version: "bioprism-neurosurgery-public-literature/0.1",
        bundle_digest: "a".repeat(64),
        query: args.query,
        total_matches: 1,
        returned_matches: 1,
        truncated: false,
        hits: [{ specialty: "glioma", pmid: "12345678", title: "Glioma study", journal: "Journal", publication_date: "2024", doi: null, source_id: "pubmed_glioma", source_uri: "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/", record_uri: "https://pubmed.ncbi.nlm.nih.gov/12345678/" }],
        abstract_count: 1,
        abstract_truncated_count: 0,
        specialty_counts: [{ specialty: "glioma", count: 1 }],
      });
      if (name === NEUROSURGERY_MISSION_TOOL && args.operation === "validate") return response(name, {
        valid: true,
        mission_id: "neurosurgical-mission-test",
        specialty: "glioma",
        status: "needs_evidence",
        human_review_required: true,
        request_digest: "q".repeat(64),
        audit_digest: "a".repeat(64),
        provider: "none",
        network: false,
      });
      if (name === NEUROSURGERY_MISSION_TOOL) return response(name, {
        schema: "bioprism-neurosurgical-research-mission/0.1",
        mission_id: "neurosurgical-mission-test",
        specialty: "glioma",
        status: "needs_evidence",
        human_review_required: true,
        provider: "none",
        network: false,
        effects: ["read_only"],
        catalogue: { specialty_count: 6, tool_count: 16 },
        real_data_query: { returned_count: 1 },
        real_data_review_queue: { bundle_digest: "b".repeat(64), provider: "none", network: false, human_review_required: true },
        real_data_evidence_packet: { bundle_digest: "b".repeat(64), provider: "none", network: false, human_review_required: true },
        public_literature_evidence_packet: { bundle_digest: "p".repeat(64), provider: "none", network: false, human_review_required: true },
        public_literature_integrity_audit: { schema_version: "bioprism-neurosurgery-public-literature-integrity-audit/0.1", bundle_digest: "p".repeat(64), requires_integrity_review: true, provider: "none", network: false, human_review_required: true },
        public_literature_review_queue: { schema_version: "bioprism-neurosurgery-public-literature-review-queue/0.1", bundle_digest: "p".repeat(64), candidate_item_count: 3, returned_item_count: 3, provider: "none", network: false, human_review_required: true },
        public_literature_workbench: { schema_version: "bioprism-neurosurgery-public-literature-workbench/0.1", bundle_digest: "p".repeat(64), specialty_count: 1, total_record_count: 25, provider: "none", network: false, synthetic_data: false, human_review_required: true },
        public_literature_portfolio: { schema_version: "bioprism-neurosurgery-public-literature-portfolio/0.1", bundle_digest: "p".repeat(64), specialty_count: 2, total_match_count: 48, provider: "none", network: false, synthetic_data: false, human_review_required: true },
        literature_link_audit: { schema_version: "bioprism-neurosurgery-literature-link-audit/0.1", provider: "none", network: false, synthetic_data: false, human_review_required: true },
        real_data_evidence_graph: { total_node_count: 88, specialty: "glioma", provider: "none", network: false },
        real_data_reasoning_context: { context_digest: "c".repeat(64), bundle_digest: "b".repeat(64), synthetic_data: false, network: false, human_review_required: true, context_text: "# AURORA REAL-GLIOMA REASONING CONTEXT" },
        research_plan: { schema_version: "bioprism-neurosurgery-research-plan/0.1", request_digest: "p".repeat(64), specialty: "glioma", tasks: [], human_review_required: true, provider: "none", network: false, effect: "read_only", limitations: [] },
        run: { steps_executed: 2 },
      });
      if (name === NEUROSURGERY_TOOL) return response(name, { status: "needs_evidence", specialty: "glioma", real_data: { record_count: 19 } });
      if (name !== NEUROSURGERY_SESSION_TOOL) throw new Error(`unexpected tool ${name}`);

      if (args.operation === "run") {
        return response(name, {
          steps_executed: 2,
          session: { status: NEUROSURGERY_SESSION_TERMINAL_STATUS, next_ordinal: 3, route: ["safety_gate", "human_review_hold"], events: [] },
          response: { status: "needs_evidence", specialty: "glioma" },
        });
      }
      if (args.operation === "start") {
        return response(name, { status: "planned", next_ordinal: 1, route: ["safety_gate", "human_review_hold"], events: [] });
      }
      if (args.operation === "advance") {
        const ordinal = args.session.next_ordinal;
        const terminal = ordinal === 2;
        return response(name, {
          ...args.session,
          status: terminal ? NEUROSURGERY_SESSION_TERMINAL_STATUS : "running",
          next_ordinal: ordinal + 1,
          events: [...(args.session.events ?? []), { ordinal }],
        });
      }
      if (args.operation === "finish") return response(name, { status: "needs_evidence", specialty: "glioma", real_data: { record_count: 19 } });
      throw new Error(`unexpected operation ${args.operation}`);
    },
  };
  return client;
}

test("provider-free facade exposes only the curated neurosurgical tools", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  assert.deepEqual((await agent.catalogue()).map((tool) => tool.name), [
    NEUROSURGERY_CATALOGUE_TOOL,
    NEUROSURGERY_INTAKE_PLAN_TOOL,
    NEUROSURGERY_INTAKE_MISSION_TOOL,
    NEUROSURGERY_INTAKE_PORTFOLIO_TOOL,
    NEUROSURGERY_EVIDENCE_AUDIT_TOOL,
    NEUROSURGERY_SPECIALTY_EVIDENCE_MAP_TOOL,
    NEUROSURGERY_EVIDENCE_PROGRAM_TOOL,
    NEUROSURGERY_CASE_ASSET_MANIFEST_TOOL,
    NEUROSURGERY_CASE_FHIR_IMPORT_TOOL,
    NEUROSURGERY_CASE_DICOM_IMPORT_TOOL,
    NEUROSURGERY_CASE_DICOM_EVIDENCE_WORKFLOW_TOOL,
    NEUROSURGERY_CASE_ASSET_REVIEW_DISPOSITION_TOOL,
    NEUROSURGERY_EVIDENCE_SYNTHESIS_TOOL,
    NEUROSURGERY_EVIDENCE_GRAPH_TOOL,
    NEUROSURGERY_GLIOMA_MOLECULAR_MAP_TOOL,
    NEUROSURGERY_REAL_DATA_COVERAGE_TOOL,
    NEUROSURGERY_REAL_DATA_COHORT_LANDSCAPE_TOOL,
    NEUROSURGERY_REAL_DATA_RECONCILIATION_TOOL,
    NEUROSURGERY_REAL_DATA_FRESHNESS_TOOL,
    NEUROSURGERY_REAL_DATA_DIFF_TOOL,
    NEUROSURGERY_REAL_DATA_REFRESH_AUDIT_TOOL,
    NEUROSURGERY_REAL_DATA_REVIEW_QUEUE_TOOL,
    NEUROSURGERY_REAL_DATA_REVIEW_DISPOSITION_TOOL,
    NEUROSURGERY_REAL_DATA_EVIDENCE_PACKET_TOOL,
    NEUROSURGERY_REAL_DATA_AUTONOMOUS_WORKFLOW_TOOL,
    NEUROSURGERY_REAL_DATA_REASONING_CONTEXT_TOOL,
    NEUROSURGERY_REAL_DATA_DRAFT_AUDIT_TOOL,
    NEUROSURGERY_PUBLIC_LITERATURE_EVIDENCE_PACKET_TOOL,
    NEUROSURGERY_PUBLIC_LITERATURE_REASONING_CONTEXT_TOOL,
    NEUROSURGERY_PUBLIC_LITERATURE_DRAFT_AUDIT_TOOL,
    NEUROSURGERY_PUBLIC_LITERATURE_MATRIX_TOOL,
    NEUROSURGERY_PUBLIC_LITERATURE_FRESHNESS_TOOL,
    NEUROSURGERY_PUBLIC_LITERATURE_REFRESH_AUDIT_TOOL,
    NEUROSURGERY_LITERATURE_LINK_AUDIT_TOOL,
    NEUROSURGERY_PUBLIC_LITERATURE_INTEGRITY_AUDIT_TOOL,
    NEUROSURGERY_PUBLIC_LITERATURE_REVIEW_QUEUE_TOOL,
    NEUROSURGERY_PUBLIC_LITERATURE_WORKBENCH_TOOL,
    NEUROSURGERY_PUBLIC_LITERATURE_PORTFOLIO_TOOL,
    NEUROSURGERY_RESEARCH_PLAN_TOOL,
    NEUROSURGERY_EVIDENCE_ACQUISITION_TOOL,
    NEUROSURGERY_RESEARCH_BRIEF_TOOL,
    NEUROSURGERY_TOOL,
    NEUROSURGERY_REAL_DATA_QUERY_TOOL,
    NEUROSURGERY_REAL_DATA_TRIAL_LANDSCAPE_TOOL,
    NEUROSURGERY_REAL_DATA_MOLECULAR_COVERAGE_TOOL,
    NEUROSURGERY_PUBLIC_LITERATURE_QUERY_TOOL,
    NEUROSURGERY_SESSION_TOOL,
    NEUROSURGERY_MISSION_TOOL,
  ]);
  assert.equal((await agent.specialtyCatalogue()).specialties[0], "glioma");
  const intake = await agent.intakePlan("Review glioma MGMT and IDH evidence");
  assert.equal(intake.selected_specialty, "glioma");
  assert.equal(intake.abstained, false);
  assert.equal(intake.provider, "none");
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_INTAKE_PLAN_TOOL);
  const mission = await agent.intakeMission(
    "Review glioma MGMT and IDH evidence",
    {},
    "glioma",
    { schema_version: "bioprism-neurosurgery-real/0.1" },
    null,
    6,
    32,
    undefined,
    { schema_version: "bioprism-neurosurgery-case-asset-manifest/0.1", specialty: "glioma", synthetic_data: false, assets: [] },
    { requested_kinds: ["imaging_series"] },
    { as_of: "2027-08-31T00:00:00Z", max_age_days: 30 },
    undefined,
    undefined,
    { report_digest: "d".repeat(64) },
  );
  assert.equal(mission.status, "ready_for_human_review");
  assert.equal(mission.provider, "none");
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_INTAKE_MISSION_TOOL);
  assert.equal(client.calls.at(-1).args.max_session_steps, 32);
  assert.deepEqual(client.calls.at(-1).args.case_asset_manifest_query.requested_kinds, ["imaging_series"]);
  assert.equal(client.calls.at(-1).args.case_asset_review_disposition.report_digest, "d".repeat(64));
  assert.equal(client.calls.at(-1).args.freshness.max_age_days, 30);
  assert.equal("question" in mission, false);
  const importedMission = await agent.intakeMission(
    "Review glioma imaging and molecular evidence",
    {},
    "glioma",
    { schema_version: "bioprism-neurosurgery-real/0.1" },
    null,
    6,
    32,
    undefined,
    undefined,
    undefined,
    undefined,
    { schema_version: "bioprism-neurosurgery-case-dicom-import/0.1" },
    { schema_version: "bioprism-neurosurgery-case-fhir-import/0.1" },
  );
  assert.equal(importedMission.status, "ready_for_human_review");
  assert.ok(client.calls.at(-1).args.case_dicom_import);
  assert.ok(client.calls.at(-1).args.case_fhir_import);
  await assert.rejects(
    () => agent.intakeMission(
      "Review glioma evidence",
      {},
      "glioma",
      undefined,
      undefined,
      6,
      32,
      undefined,
      { schema_version: "bioprism-neurosurgery-case-asset-manifest/0.1" },
      undefined,
      undefined,
      { schema_version: "bioprism-neurosurgery-case-dicom-import/0.1" },
    ),
    ArgumentError,
  );
  await assert.rejects(
    () => agent.intakeMission("Review glioma evidence", {}, "glioma", undefined, undefined, 6, 32, undefined, undefined, { requested_kinds: ["imaging_series"] }),
    ArgumentError,
  );
  await assert.rejects(
    () => agent.intakePortfolio("Review glioma evidence", {}, "glioma", undefined, undefined, 6, false, 4, 4, 8, 16, undefined, undefined, undefined, undefined, { report_digest: "d".repeat(64) }),
    ArgumentError,
  );
  await assert.rejects(
    () => agent.intakeMission("Review glioma evidence", {}, "glioma", undefined, undefined, 6, 32, undefined, undefined, undefined, { as_of: "2026-02-30T00:00:00Z" }),
    ArgumentError,
  );
  const caseRequest = {
    case_id: "case-deidentified-001",
    specialty: "glioma",
    request_use: "research_synthesis",
    question: "transient case question",
    observations: [],
  };
  await agent.intakeMission(
    "Route this glioma case through the evidence workflow",
    {},
    "glioma",
    { schema_version: "bioprism-neurosurgery-real/0.1" },
    null,
    6,
    32,
    caseRequest,
  );
  assert.deepEqual(client.calls.at(-1).args.case_request, caseRequest);
  const selectedPortfolio = await agent.intakePortfolio(
    "Review glioma evidence",
    {},
    "glioma",
    { schema_version: "bioprism-neurosurgery-real/0.1" },
    { schema_version: "bioprism-neurosurgery-public-literature/0.1" },
    6,
    false,
    4,
    4,
    8,
    16,
    undefined,
    { schema_version: "bioprism-neurosurgery-case-asset-manifest/0.1", specialty: "glioma", synthetic_data: false, assets: [] },
    { requested_kinds: ["pathology_report"] },
    { as_of: "2027-08-31T00:00:00Z" },
    { report_digest: "d".repeat(64), decisions: [] },
  );
  assert.equal(selectedPortfolio.status, "ready_for_human_review");
  assert.deepEqual(client.calls.at(-1).args.case_asset_manifest_query.requested_kinds, ["pathology_report"]);
  assert.equal(client.calls.at(-1).args.freshness.max_age_days, 365);
  assert.equal(client.calls.at(-1).args.case_asset_review_disposition.report_digest, "d".repeat(64));
  const intakePortfolio = await agent.intakePortfolio(
    "Review all neurosurgical evidence lanes",
    {},
    undefined,
    null,
    { schema_version: "bioprism-neurosurgery-public-literature/0.1" },
    6,
    true,
    4,
    4,
    8,
    16,
    caseRequest,
  );
  assert.equal(intakePortfolio.status, "ready_for_human_review");
  assert.equal(intakePortfolio.selected_specialties.length, 6);
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_INTAKE_PORTFOLIO_TOOL);
  assert.equal(client.calls.at(-1).args.include_all_specialties, true);
  assert.equal(client.calls.at(-1).args.max_candidates, 6);
  await assert.rejects(() => agent.intakePlan("   "), ArgumentError);
  const audit = await agent.auditEvidence({ specialty: "encephalocele", request_use: "research_synthesis" });
  assert.equal(audit.coverage_complete, false);
  const specialtyMap = await agent.specialtyEvidenceMap({ specialty: "glioma", request_use: "research_synthesis" });
  assert.equal(specialtyMap.schema_version, "bioprism-neurosurgery-specialty-evidence-map/0.1");
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_SPECIALTY_EVIDENCE_MAP_TOOL);
  assert.equal(client.calls.at(-1).args.request.specialty, "glioma");
  const assetManifest = await agent.caseAssetManifest(
    { specialty: "glioma", request_use: "research_synthesis" },
    {
      schema_version: "bioprism-neurosurgery-case-asset-manifest/0.1",
      specialty: "glioma",
      synthetic_data: false,
      assets: [],
    },
    {},
    ["imaging_series"],
    16,
  );
  assert.equal(assetManifest.schema_version, "bioprism-neurosurgery-case-asset-manifest/0.1");
  assert.equal(assetManifest.provider, "none");
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_CASE_ASSET_MANIFEST_TOOL);
  assert.deepEqual(client.calls.at(-1).args.query.requested_kinds, ["imaging_series"]);
  const fhirReport = await agent.caseFhirImport(
    { specialty: "glioma", request_use: "research_synthesis" },
    {
      schema_version: "bioprism-neurosurgery-case-fhir-import/0.1",
      specialty: "glioma",
      deidentified: true,
      synthetic_data: false,
      source_id: "export-a",
      bundle: { resourceType: "Bundle", entry: [] },
    },
  );
  assert.equal(fhirReport.schema_version, "bioprism-neurosurgery-case-fhir-import/0.1");
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_CASE_FHIR_IMPORT_TOOL);
  assert.equal(client.calls.at(-1).args.import.source_id, "export-a");
  const dicomReport = await agent.caseDicomImport(
    { specialty: "glioma", request_use: "research_synthesis" },
    {
      schema_version: "bioprism-neurosurgery-case-dicom-import/0.1",
      specialty: "glioma",
      deidentified: true,
      synthetic_data: false,
      source_id: "dicom-export-a",
      datasets: [],
    },
  );
  assert.equal(dicomReport.schema_version, "bioprism-neurosurgery-case-dicom-import/0.1");
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_CASE_DICOM_IMPORT_TOOL);
  assert.equal(client.calls.at(-1).args.import.source_id, "dicom-export-a");
  const dicomWorkflow = await agent.caseDicomEvidenceWorkflow(
    { specialty: "glioma", request_use: "research_synthesis" },
    {
      schema_version: "bioprism-neurosurgery-case-dicom-import/0.1",
      specialty: "glioma",
      deidentified: true,
      synthetic_data: false,
      source_id: "dicom-export-a",
      datasets: [],
    },
    {},
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    null,
    { max_acquisition_steps: 4, real_data_reasoning_context: { max_chars: 10000 } },
  );
  assert.equal(dicomWorkflow.schema_version, "bioprism-neurosurgery-case-dicom-evidence-workflow/0.1");
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_CASE_DICOM_EVIDENCE_WORKFLOW_TOOL);
  assert.equal(client.calls.at(-1).args.query.max_acquisition_steps, 4);
  assert.equal(client.calls.at(-1).args.query.real_data_reasoning_context.max_chars, 10000);
  const assetDisposition = await agent.caseAssetReviewDisposition(
    assetManifest,
    [{ sequence: 1, disposition: "reviewed", reviewer_id: "reviewer-a" }],
  );
  assert.equal(assetDisposition.schema_version, "bioprism-neurosurgery-case-asset-review-disposition/0.1");
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_CASE_ASSET_REVIEW_DISPOSITION_TOOL);
  assert.deepEqual(client.calls.at(-1).args.decisions, [
    { sequence: 1, disposition: "reviewed", reviewer_id: "reviewer-a" },
  ]);
  const evidenceProgram = await agent.evidenceProgramWithCaseAssets(
    { specialty: "glioma", request_use: "research_synthesis" },
    {},
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    { schema_version: "bioprism-neurosurgery-public-literature/0.1", synthetic_data: false },
    { schema_version: "bioprism-neurosurgery-case-asset-manifest/0.1", specialty: "glioma", synthetic_data: false, assets: [] },
    { requested_kinds: ["imaging_series"] },
    {},
  );
  assert.equal(evidenceProgram.schema_version, "bioprism-neurosurgery-evidence-program/0.1");
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_EVIDENCE_PROGRAM_TOOL);
  assert.deepEqual(client.calls.at(-1).args.case_asset_manifest_query.requested_kinds, ["imaging_series"]);
  assert.equal(evidenceProgram.lanes[0].tracks[0].review_worklist[0].code, "asset_class_missing");
  await assert.rejects(
    () => agent.caseAssetManifest(
      { specialty: "glioma", request_use: "research_synthesis" },
      { schema_version: "bioprism-neurosurgery-case-asset-manifest/0.1", specialty: "glioma", assets: [] },
      {},
      ["imaging_series", "imaging_series"],
    ),
    ArgumentError,
  );
  const synthesisRequest = {
    case_id: "case-synthesis-001",
    specialty: "glioma",
    request_use: "research_synthesis",
    question: "Align public glioma evidence",
    observations: [],
  };
  const synthesis = await agent.evidenceSynthesis(
    synthesisRequest,
    {},
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    { schema_version: "bioprism-neurosurgery-public-literature/0.1", synthetic_data: false },
    { max_references: 12, include_source_text: true },
  );
  assert.equal(synthesis.schema_version, "bioprism-neurosurgery-evidence-synthesis/0.1");
  assert.equal(synthesis.provider, "none");
  assert.equal(synthesis.synthetic_data, false);
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_EVIDENCE_SYNTHESIS_TOOL);
  assert.equal(client.calls.at(-1).args.query.max_references, 12);
  assert.equal(client.calls.at(-1).args.query.include_source_text, true);
  await assert.rejects(
    () => agent.evidenceSynthesis(
      synthesisRequest,
      {},
      undefined,
      undefined,
      {},
      undefined,
      { requested_kinds: ["imaging_series"] },
    ),
    ArgumentError,
  );
  const directManifest = {
    schema_version: "bioprism-neurosurgery-case-asset-manifest/0.1",
    specialty: "glioma",
    synthetic_data: false,
    assets: [],
  };
  await agent.evidenceSynthesis(
    synthesisRequest,
    {},
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    undefined,
    {},
    directManifest,
    { requested_kinds: ["imaging_series"] },
    {
      schema_version: "bioprism-neurosurgery-case-asset-review-disposition/0.1",
      report_digest: "d".repeat(64),
      disposition_digest: "x".repeat(64),
      candidate_item_count: 0,
      returned_item_count: 0,
      omitted_item_count: 0,
      submitted_decision_count: 0,
      accepted_decision_count: 0,
      resolved_decision_count: 0,
      unresolved_decision_count: 0,
      undecided_returned_item_count: 0,
      pending_item_count: 0,
      decisions: [],
      unresolved_sequences: [],
      undecided_sequences: [],
      provenance_bound: true,
      synthetic_data: false,
      human_review_required: true,
      provider: "none",
      network: false,
      effect: "read_only",
      limitations: [],
    },
  );
  assert.deepEqual(client.calls.at(-1).args.case_asset_manifest, directManifest);
  assert.deepEqual(client.calls.at(-1).args.case_asset_manifest_query.requested_kinds, ["imaging_series"]);
  assert.equal(client.calls.at(-1).args.case_asset_review_disposition.report_digest, "d".repeat(64));
  const molecularMap = await agent.gliomaMolecularMap(
    synthesisRequest,
    {},
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    { schema_version: "bioprism-neurosurgery-public-literature/0.1", synthetic_data: false },
    { markers: ["idh1_mutation", "mgmt_promoter_methylation"], max_hits_per_marker: 4 },
  );
  assert.equal(molecularMap.schema_version, "bioprism-neurosurgery-glioma-molecular-map/0.1");
  assert.equal(molecularMap.provider, "none");
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_GLIOMA_MOLECULAR_MAP_TOOL);
  assert.deepEqual(client.calls.at(-1).args.query.markers, ["idh1_mutation", "mgmt_promoter_methylation"]);
  const temporal = await agent.temporalAudit({ specialty: "encephalocele", request_use: "research_synthesis" });
  assert.equal(temporal.schema_version, "bioprism-neurosurgery-temporal-alignment/0.1");
  assert.equal(temporal.status, "unavailable");
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_EVIDENCE_AUDIT_TOOL);
  const researchPlan = await agent.planResearch(
    { specialty: "encephalocele", request_use: "research_synthesis" },
    {},
    undefined,
    undefined,
    3,
    2,
  );
  assert.equal(researchPlan.schema_version, "bioprism-neurosurgery-research-plan/0.1");
  assert.equal(researchPlan.human_review_required, true);
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_RESEARCH_PLAN_TOOL);
  const acquisition = await agent.evidenceAcquisition(
    { specialty: "glioma", request_use: "research_synthesis" },
    {},
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    { schema_version: "bioprism-neurosurgery-public-literature/0.1", synthetic_data: false },
    { max_steps: 8, max_references_per_step: 2, freshness: { as_of: "2026-08-30T00:00:00Z", max_age_days: 30 } },
  );
  assert.equal(acquisition.schema_version, "bioprism-neurosurgery-evidence-acquisition/0.1");
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_EVIDENCE_ACQUISITION_TOOL);
  assert.equal(client.calls.at(-1).args.query.max_steps, 8);
  assert.equal(client.calls.at(-1).args.query.freshness.max_age_days, 30);
  const acquisitionManifest = {
    schema_version: "bioprism-neurosurgery-case-asset-manifest/0.1",
    specialty: "glioma",
    synthetic_data: false,
    assets: [],
  };
  await agent.evidenceAcquisition(
    { specialty: "glioma", request_use: "research_synthesis" },
    {},
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    undefined,
    {},
    acquisitionManifest,
    { requested_kinds: ["imaging_series"] },
  );
  assert.deepEqual(client.calls.at(-1).args.case_asset_manifest, acquisitionManifest);
  assert.deepEqual(client.calls.at(-1).args.case_asset_manifest_query.requested_kinds, ["imaging_series"]);
  const started = await agent.evidenceAcquisitionStart(
    { specialty: "glioma", request_use: "research_synthesis" },
    {},
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    undefined,
    { max_steps: 2 },
  );
  assert.equal(started.schema_version, "bioprism-neurosurgery-evidence-acquisition-session/0.1");
  const advanced = await agent.evidenceAcquisitionAdvance(
    { specialty: "glioma", request_use: "research_synthesis" },
    started.session,
    {},
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    undefined,
    { max_steps: 2 },
    1,
  );
  assert.equal(advanced.complete, true);
  assert.equal(client.calls.at(-1).args.operation, "advance");
  const finished = await agent.evidenceAcquisitionFinish(
    { specialty: "glioma", request_use: "research_synthesis" },
    advanced.session,
    {},
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    undefined,
    { max_steps: 2 },
  );
  assert.equal(finished.schema_version, "bioprism-neurosurgery-evidence-acquisition-execution/0.1");
  await assert.rejects(
    () => agent.evidenceAcquisition({ specialty: "glioma", request_use: "research_synthesis" }, {}, undefined, undefined, { max_steps: 0 }),
    /query\.max_steps/,
  );
  const researchBrief = await agent.researchBrief(
    { specialty: "glioma", request_use: "research_synthesis" },
    {},
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    undefined,
    { focus_terms: ["glioblastoma"], max_topics: 4, max_records_per_topic: 2, include_abstracts: true },
  );
  assert.equal(researchBrief.schema_version, "bioprism-neurosurgery-research-brief/0.1");
  assert.equal(researchBrief.source, "real_glioma");
  assert.equal(researchBrief.provider, "none");
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_RESEARCH_BRIEF_TOOL);
  assert.equal(client.calls.at(-1).args.query.focus_terms[0], "glioblastoma");
  await assert.rejects(
    () => agent.researchBrief({ specialty: "glioma", request_use: "research_synthesis" }),
    ArgumentError,
  );
  const graph = await agent.evidenceGraph(
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    { root_record_id: "24120142", root_record_kind: "literature_article", max_nodes: 16, max_edges: 32 },
  );
  assert.equal(graph.schema_version, "bioprism-neurosurgery-evidence-graph/0.1");
  assert.equal(graph.human_review_required, true);
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_EVIDENCE_GRAPH_TOOL);
  assert.equal(client.calls.at(-1).args.query.max_edges, 32);
  const coverage = await agent.realDataCoverage(
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    { record_kind: "clinical_trial", source_id: "clinicaltrials_glioma_2026-08-30", from_year: 2020, to_year: 2025 },
  );
  assert.equal(coverage.schema_version, "bioprism-neurosurgery-real-data-coverage/0.1");
  assert.equal(coverage.matched_record_count, 4);
  assert.equal(coverage.synthetic_data, false);
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_REAL_DATA_COVERAGE_TOOL);
  assert.equal(client.calls.at(-1).args.query.from_year, 2020);
  const reconciliation = await agent.realDataReconciliation(
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    { max_issues: 12 },
  );
  assert.equal(reconciliation.schema_version, "bioprism-neurosurgery-real-data-reconciliation/0.1");
  assert.equal(reconciliation.synthetic_data, false);
  assert.equal(reconciliation.provider, "none");
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_REAL_DATA_RECONCILIATION_TOOL);
  assert.equal(client.calls.at(-1).args.query.max_issues, 12);
  await assert.rejects(
    () => agent.realDataReconciliation({ schema_version: "bioprism-neurosurgery-real/0.1" }, { max_issues: 0 }),
    ArgumentError,
  );
  const freshness = await agent.realDataFreshness(
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    { as_of: "2026-08-31T00:00:00Z", max_age_days: 30 },
  );
  assert.equal(freshness.schema_version, "bioprism-neurosurgery-real-data-freshness/0.1");
  assert.equal(freshness.status, "stale");
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_REAL_DATA_FRESHNESS_TOOL);
  assert.equal(client.calls.at(-1).args.query.max_age_days, 30);
  await assert.rejects(
    () => agent.realDataFreshness({ schema_version: "bioprism-neurosurgery-real/0.1" }, { as_of: "2026-02-30T00:00:00Z" }),
    ArgumentError,
  );
  const diff = await agent.realDataDiff(
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    { record_kind: "clinical_trial", source_id: "clinicaltrials_glioma_2026-08-30", max_changes: 4 },
  );
  assert.equal(diff.schema_version, "bioprism-neurosurgery-real-data-diff/0.1");
  assert.equal(diff.record_counts.changed, 1);
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_REAL_DATA_DIFF_TOOL);
  assert.equal(client.calls.at(-1).args.query.max_changes, 4);
  const refresh = await agent.realDataRefreshAudit(
    { specialty: "glioma", request_use: "research_synthesis" },
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    { brief: { focus_terms: ["MGMT"] } },
  );
  assert.equal(refresh.schema_version, "bioprism-neurosurgery-real-data-refresh-audit/0.1");
  assert.equal(refresh.source_identity_stable, true);
  assert.equal(refresh.provider, "none");
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_REAL_DATA_REFRESH_AUDIT_TOOL);
  assert.deepEqual(client.calls.at(-1).args.query.brief.focus_terms, ["MGMT"]);
  const reviewQueue = await agent.realDataReviewQueue(
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    { record_kind: "portal_study", source_id: "cbioportal_glioma_2026-08-30", max_items: 2 },
  );
  assert.equal(reviewQueue.schema_version, "bioprism-neurosurgery-real-data-review-queue/0.1");
  assert.equal(reviewQueue.candidate_item_count, 15);
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_REAL_DATA_REVIEW_QUEUE_TOOL);
  assert.equal(client.calls.at(-1).args.query.max_items, 2);
  const disposition = await agent.realDataReviewDisposition(
    reviewQueue,
    [{ task_id: "real-review-missing_portal_publication_link-portal_study-gbm_tcga_gdc", disposition: "reviewed", reviewer_id: "ts-test" }],
  );
  assert.equal(disposition.schema_version, "bioprism-neurosurgery-real-data-review-disposition/0.1");
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_REAL_DATA_REVIEW_DISPOSITION_TOOL);
  assert.equal(client.calls.at(-1).args.decisions.length, 1);
  const packet = await agent.realDataEvidencePacket(
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    { query: { text: "glioblastoma", limit: 4 }, graph: { max_nodes: 8, max_edges: 12 }, review_queue: { max_items: 3 }, freshness: { as_of: "2027-08-31T00:00:00Z", max_age_days: 30 } },
  );
  assert.equal(packet.schema_version, "bioprism-neurosurgery-real-data-evidence-packet/0.4");
  assert.equal(packet.cohort_landscape?.returned_project_count, 1);
  assert.equal(packet.cohort_landscape?.project_rows[0]?.project_id, "TCGA-GBM");
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_REAL_DATA_EVIDENCE_PACKET_TOOL);
  assert.equal(client.calls.at(-1).args.query.query.limit, 4);
  assert.equal(client.calls.at(-1).args.query.freshness.as_of, "2027-08-31T00:00:00Z");
  const workflow = await agent.realDataAutonomousWorkflow(
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    { packet: { review_queue: { max_items: 8 } }, max_actions: 12 },
  );
  assert.equal(workflow.schema_version, "bioprism-neurosurgery-real-data-autonomous-workflow/0.1");
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_REAL_DATA_AUTONOMOUS_WORKFLOW_TOOL);
  assert.equal(client.calls.at(-1).args.query.max_actions, 12);
  assert.equal(client.calls.at(-1).args.query.packet.review_queue.max_items, 8);
  const context = await agent.realDataReasoningContext(
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    { packet: { query: { text: "glioblastoma", limit: 2 } }, max_chars: 6000, include_abstracts: true },
  );
  assert.equal(context.schema_version, "bioprism-neurosurgery-real-data-reasoning-context/0.1");
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_REAL_DATA_REASONING_CONTEXT_TOOL);
  assert.equal(client.calls.at(-1).args.query.packet.query.limit, 2);
  assert.equal(client.calls.at(-1).args.query.max_chars, 6000);
  assert.equal(client.calls.at(-1).args.query.include_abstracts, true);
  const draft = await agent.realDataDraftAudit(
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    [{
      claim_id: "trial-metadata",
      kind: "source_observation",
      scope: "public_record_metadata",
      text: "The packet contains a public registry record.",
      citations: [{ record_kind: "clinical_trial", record_id: "NCT00005955" }],
    }],
    { query: { text: "glioblastoma", limit: 4 } },
  );
  assert.equal(draft.schema_version, "bioprism-neurosurgery-real-data-draft-audit/0.1");
  assert.equal(draft.status, "grounded_for_human_review");
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_REAL_DATA_DRAFT_AUDIT_TOOL);
  assert.equal(client.calls.at(-1).args.claims.length, 1);
  assert.equal(client.calls.at(-1).args.query.query.limit, 4);
  const literaturePacket = await agent.publicLiteratureEvidencePacket(
    { schema_version: "bioprism-neurosurgery-public-literature/0.1", synthetic_data: false },
    { specialty: "chiari_malformation", limit: 2 },
    {},
    { as_of: "2027-08-31T00:00:00Z", max_age_days: 30 },
  );
  assert.equal(literaturePacket.schema_version, "bioprism-neurosurgery-public-literature-evidence-packet/0.1");
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_PUBLIC_LITERATURE_EVIDENCE_PACKET_TOOL);
  assert.equal(client.calls.at(-1).args.query.query.limit, 2);
  assert.equal(client.calls.at(-1).args.query.freshness.max_age_days, 30);
  const literatureContext = await agent.publicLiteratureReasoningContext(
    { schema_version: "bioprism-neurosurgery-public-literature/0.1", synthetic_data: false },
    { packet: { query: { specialty: "chiari_malformation", limit: 2 } }, max_chars: 6000, include_abstracts: true },
  );
  assert.equal(literatureContext.schema_version, "bioprism-neurosurgery-public-literature-reasoning-context/0.1");
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_PUBLIC_LITERATURE_REASONING_CONTEXT_TOOL);
  assert.equal(client.calls.at(-1).args.query.packet.query.limit, 2);
  assert.equal(client.calls.at(-1).args.query.max_chars, 6000);
  assert.equal(client.calls.at(-1).args.query.include_abstracts, true);
  const literatureDraft = await agent.publicLiteratureDraftAudit(
    { schema_version: "bioprism-neurosurgery-public-literature/0.1", synthetic_data: false },
    [{
      claim_id: "chiari-citation",
      kind: "source_observation",
      scope: "citation_metadata",
      text: "The packet contains a source-linked PMID.",
      citations: [{ record_kind: "literature_article", record_id: "12345678" }],
    }],
    { specialty: "chiari_malformation", limit: 1 },
  );
  assert.equal(literatureDraft.schema_version, "bioprism-neurosurgery-public-literature-draft-audit/0.1");
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_PUBLIC_LITERATURE_DRAFT_AUDIT_TOOL);
  assert.equal(client.calls.at(-1).args.query.query.limit, 1);
  const literatureMatrix = await agent.publicLiteratureMatrix(
    { schema_version: "bioprism-neurosurgery-public-literature/0.1", synthetic_data: false },
    { specialties: ["glioma", "chiari_malformation"], query: { text: "glioma", limit: 2 } },
  );
  assert.equal(literatureMatrix.schema_version, "bioprism-neurosurgery-public-literature-matrix/0.1");
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_PUBLIC_LITERATURE_MATRIX_TOOL);
  assert.deepEqual(client.calls.at(-1).args.query.specialties, ["glioma", "chiari_malformation"]);
  assert.equal(client.calls.at(-1).args.query.query.limit, 2);
  const literatureFreshness = await agent.publicLiteratureFreshness(
    { schema_version: "bioprism-neurosurgery-public-literature/0.1", synthetic_data: false },
    { as_of: "2026-08-31T00:00:00Z" },
  );
  assert.equal(literatureFreshness.status, "current");
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_PUBLIC_LITERATURE_FRESHNESS_TOOL);
  const literatureRefresh = await agent.publicLiteratureRefreshAudit(
    { schema_version: "bioprism-neurosurgery-public-literature/0.1", synthetic_data: false },
    { schema_version: "bioprism-neurosurgery-public-literature/0.1", synthetic_data: false },
    { max_source_changes: 8, max_record_changes: 16 },
  );
  assert.equal(literatureRefresh.schema_version, "bioprism-neurosurgery-public-literature-refresh-audit/0.1");
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_PUBLIC_LITERATURE_REFRESH_AUDIT_TOOL);
  assert.equal(client.calls.at(-1).args.query.max_record_changes, 16);
  const literatureLink = await agent.literatureLinkAudit(
    { schema_version: "bioprism-neurosurgery-real-glioma/0.1" },
    { schema_version: "bioprism-neurosurgery-public-literature/0.1" },
    { max_links: 8, max_unmatched_ids: 16 },
  );
  assert.equal(literatureLink.schema_version, "bioprism-neurosurgery-literature-link-audit/0.1");
  assert.equal(literatureLink.counts.linked_real_records, 12);
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_LITERATURE_LINK_AUDIT_TOOL);
  assert.equal(client.calls.at(-1).args.query.max_links, 8);
  const literatureIntegrity = await agent.publicLiteratureIntegrityAudit(
    { schema_version: "bioprism-neurosurgery-public-literature/0.1" },
    { specialties: ["glioma"], max_issues: 8 },
  );
  assert.equal(literatureIntegrity.schema_version, "bioprism-neurosurgery-public-literature-integrity-audit/0.1");
  assert.equal(literatureIntegrity.counts.missing_abstract_count, 7);
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_PUBLIC_LITERATURE_INTEGRITY_AUDIT_TOOL);
  assert.equal(client.calls.at(-1).args.query.max_issues, 8);
  const literatureQueue = await agent.publicLiteratureReviewQueue(
    { schema_version: "bioprism-neurosurgery-public-literature/0.1" },
    { specialties: ["glioma"], max_items: 8 },
  );
  assert.equal(literatureQueue.schema_version, "bioprism-neurosurgery-public-literature-review-queue/0.1");
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_PUBLIC_LITERATURE_REVIEW_QUEUE_TOOL);
  assert.equal(client.calls.at(-1).args.query.max_items, 8);
  const workbench = await agent.publicLiteratureWorkbench(
    { schema_version: "bioprism-neurosurgery-public-literature/0.1", synthetic_data: false },
    {
      specialties: ["glioma"],
      max_issues_per_lane: 8,
      freshness: { as_of: "2027-08-31T00:00:00Z", max_age_days: 30 },
    },
  );
  assert.equal(workbench.schema_version, "bioprism-neurosurgery-public-literature-workbench/0.1");
  assert.equal(workbench.lanes[0].profile.specialty, "glioma");
  assert.equal(workbench.total_record_count, 25);
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_PUBLIC_LITERATURE_WORKBENCH_TOOL);
  assert.equal(client.calls.at(-1).args.query.max_issues_per_lane, 8);
  assert.equal(client.calls.at(-1).args.query.freshness.max_age_days, 30);
  await assert.rejects(
    () => agent.publicLiteratureWorkbench({}, { specialties: ["glioma", "glioma"] }),
    ArgumentError,
  );
  const portfolio = await agent.publicLiteraturePortfolio(
    { schema_version: "bioprism-neurosurgery-public-literature/0.1", synthetic_data: false },
    {
      specialties: ["glioma", "chiari_malformation"],
      text: "glioblastoma",
      from_date: "2020-01-01",
      to_date: "2026-01-01",
      max_hits_per_lane: 2,
      max_review_items_per_lane: 2,
      max_issues_per_lane: 8,
      freshness: { as_of: "2027-08-31T00:00:00Z", max_age_days: 30 },
    },
  );
  assert.equal(portfolio.schema_version, "bioprism-neurosurgery-public-literature-portfolio/0.1");
  assert.equal(portfolio.lanes[0].specialty, "glioma");
  assert.equal(portfolio.total_returned_count, 2);
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_PUBLIC_LITERATURE_PORTFOLIO_TOOL);
  assert.equal(client.calls.at(-1).args.query.max_hits_per_lane, 2);
  await assert.rejects(
    () => agent.publicLiteraturePortfolio({}, { max_hits_per_lane: 0 }),
    ArgumentError,
  );
  await assert.rejects(
    () => agent.publicLiteraturePortfolio({}, { from_date: "2026-02-30" }),
    ArgumentError,
  );
});

test("facade validates bounded real-data queries and passes population evidence through", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const bundle = { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false, sources: [] };
  const result = await agent.queryRealData(bundle, {
    text: "TCGA",
    publication_type: "systematic review",
    mesh_term: "glioblastoma",
    publication_date_from: "2019-01-01",
    publication_date_to: "2019-12-31",
    record_kind: "literature_article",
    source_id: "pubmed_glioblastoma",
    related_record_id: "gbm_tcga_pub2013",
    limit: 8,
  });
  assert.equal(result.returned_count, 1);
  const queryCall = client.calls.at(-1);
  assert.equal(queryCall.name, NEUROSURGERY_REAL_DATA_QUERY_TOOL);
  assert.equal(queryCall.args.query.limit, 8);
  assert.equal(queryCall.args.query.record_kind, "literature_article");
  assert.equal(queryCall.args.query.related_record_id, "gbm_tcga_pub2013");
  assert.equal(queryCall.args.query.publication_type, "systematic review");
  assert.equal(queryCall.args.query.mesh_term, "glioblastoma");
  assert.equal(queryCall.args.query.publication_date_from, "2019-01-01");
  assert.equal(queryCall.args.query.publication_date_to, "2019-12-31");
  await assert.rejects(
    () => agent.queryRealData(bundle, {
      publication_date_from: "2020-01-01",
      publication_date_to: "2019-12-31",
    }),
    ArgumentError,
  );
  await agent.queryRealData(bundle, { genomic_data_type: "Annotated Somatic Mutation", limit: 4 });
  assert.equal(client.calls.at(-1).args.query.genomic_data_type, "Annotated Somatic Mutation");
  const landscape = await agent.realDataTrialLandscape(bundle, {
    query: {
      trial_phase: "phase2",
      trial_updated_from: "2023-01-01",
      trial_updated_to: "2024-12-31",
    },
    max_interventions: 8,
  });
  assert.equal(landscape.returned_trial_count, 2);
  const landscapeCall = client.calls.at(-1);
  assert.equal(landscapeCall.name, NEUROSURGERY_REAL_DATA_TRIAL_LANDSCAPE_TOOL);
  assert.equal(landscapeCall.args.query.query.trial_phase, "phase2");
  assert.equal(landscapeCall.args.query.max_interventions, 8);
  const cohorts = await agent.realDataCohortLandscape(bundle, {
    query: { genomic_data_type: "Aligned Reads", limit: 8 },
    max_projects: 4,
  });
  assert.equal(cohorts.returned_project_count, 2);
  const cohortCall = client.calls.at(-1);
  assert.equal(cohortCall.name, NEUROSURGERY_REAL_DATA_COHORT_LANDSCAPE_TOOL);
  assert.equal(cohortCall.args.query.query.genomic_data_type, "Aligned Reads");
  assert.equal(cohortCall.args.query.max_projects, 4);
  await assert.rejects(
    () => agent.realDataCohortLandscape(bundle, { query: { record_kind: "clinical_trial" } }),
    ArgumentError,
  );
  const molecular = await agent.realDataMolecularCoverage(bundle, {
    query: {
      molecular_alteration_type: "mutation_extended",
      molecular_datatype: "maf",
    },
    max_studies: 8,
  });
  assert.equal(molecular.returned_profile_count, 6);
  const molecularCall = client.calls.at(-1);
  assert.equal(molecularCall.name, NEUROSURGERY_REAL_DATA_MOLECULAR_COVERAGE_TOOL);
  assert.equal(molecularCall.args.query.query.molecular_datatype, "maf");
  assert.equal(molecularCall.args.query.max_studies, 8);
  await assert.rejects(
    () => agent.realDataMolecularCoverage(bundle, { query: { record_kind: "clinical_trial" } }),
    ArgumentError,
  );
  await assert.rejects(
    () => agent.realDataMolecularCoverage(bundle, { query: { genomic_data_type: "Aligned Reads" } }),
    ArgumentError,
  );
  await assert.rejects(
    () => agent.realDataTrialLandscape(bundle, {
      query: { trial_updated_from: "2025-01-01", trial_updated_to: "2024-01-01" },
    }),
    ArgumentError,
  );
  await assert.rejects(
    () => agent.realDataTrialLandscape(bundle, { query: { record_kind: "literature_article" } }),
    ArgumentError,
  );
  const report = await agent.plan({ use: "research_synthesis", specialty: "glioma" }, {}, bundle);
  assert.equal(report.real_data.record_count, 19);
  await assert.rejects(() => agent.queryRealData(bundle, { limit: 129 }), ArgumentError);
});

test("grounded real-data bridge requires a keyless provider and audits structured claims", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  runtime.registerInMemoryProvider("ollama", () => ({
    structured: {
      answer: "The snapshot contains public aggregate records for reviewer inspection.",
      unknowns: ["Applicability to an individual case is not established."],
      claims: [{
        claim_id: "claim-1",
        kind: "population_summary",
        scope: "population_aggregate",
        text: "The supplied public snapshot contains aggregate glioma records.",
        citations: [{ record_kind: "genomic_project", record_id: "TCGA-GBM" }],
      }],
    },
  }));
  const bundle = { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false, sources: [] };
  const result = await agent.groundedRealDataResearch(
    "Summarize the available glioma population metadata.", bundle, runtime, "ollama", "llama3.1",
    {
      approveProviderCall: true,
      includeAbstracts: false,
      freshness: { as_of: "2026-08-31T00:00:00Z", max_age_days: 365 },
      realDataQuery: { record_kind: "genomic_project", genomic_data_type: "Annotated Somatic Mutation", limit: 1 },
    },
  );
  assert.equal(result.status, "grounded_for_human_review");
  assert.equal(result.transport, "in_memory");
  assert.equal(result.audit.status, "grounded_for_human_review");
  assert.equal(result.human_review_required, true);
  assert.equal(client.calls.filter((call) => call.name === NEUROSURGERY_REAL_DATA_REASONING_CONTEXT_TOOL).length, 1);
  assert.equal(client.calls.filter((call) => call.name === NEUROSURGERY_REAL_DATA_DRAFT_AUDIT_TOOL).length, 1);
  const realContextCall = client.calls.find((call) => call.name === NEUROSURGERY_REAL_DATA_REASONING_CONTEXT_TOOL);
  assert.equal(realContextCall.args.query.packet.freshness.max_age_days, 365);
  assert.deepEqual(realContextCall.args.query.packet.query, {
    record_kind: "genomic_project",
    genomic_data_type: "Annotated Somatic Mutation",
    limit: 1,
    text: "Summarize the available glioma population metadata.",
  });
  await assert.rejects(
    () => agent.groundedRealDataResearch(
      "Summarize the available glioma population metadata.", bundle, runtime, "ollama", "llama3.1",
      { approveProviderCall: true, realDataQuery: { publication_date_from: "2026-02-30" } },
    ),
    ArgumentError,
  );
  await assert.rejects(
    () => agent.groundedRealDataResearch("summarize", bundle, runtime, "ollama", "llama3.1"),
    ArgumentError,
  );
});

test("grounded real-data tool loop executes only snapshot search and closes citations", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let turns = 0;
  runtime.registerInMemoryProvider("ollama", (request) => {
    turns += 1;
    if (turns === 1) {
      return { toolCalls: [{ id: "search-1", name: NEUROSURGERY_GROUNDED_REAL_DATA_PROVIDER_TOOL, arguments: { text: "trial", limit: 1 } }] };
    }
    return {
      structured: {
        answer: "The tool returned one source-linked trial row.",
        unknowns: [],
        claims: [{
          claim_id: "tool-trial",
          kind: "source_observation",
          scope: "public_record_metadata",
          text: "A bounded query returned a clinical-trial metadata row.",
          citations: [{ record_kind: "clinical_trial", record_id: "TOOL-TRIAL" }],
        }],
      },
    };
  });
  const result = await agent.groundedRealDataResearch(
    "Find glioma trial metadata.",
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    runtime,
    "ollama",
    "llama3.1",
    { approveProviderCall: true, toolLoop: true },
  );
  assert.deepEqual(result.tool_loop, { status: "completed", turns: 2, tool_calls: 1 });
  assert.equal(result.tool_trace[0].tool, NEUROSURGERY_GROUNDED_REAL_DATA_PROVIDER_TOOL);
  assert.equal(result.tool_trace[0].query.text, undefined);
  assert.equal(result.tool_trace[0].query.text_bytes, new TextEncoder().encode("trial").byteLength);
  assert.equal(result.audit.status, "grounded_for_human_review");
  assert.deepEqual(client.calls.map((call) => call.name), [
    NEUROSURGERY_REAL_DATA_REASONING_CONTEXT_TOOL,
    NEUROSURGERY_REAL_DATA_QUERY_TOOL,
    NEUROSURGERY_REAL_DATA_DRAFT_AUDIT_TOOL,
  ]);
});

test("grounded real-data tool loop exposes deterministic topic brief", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let turns = 0;
  let continuationRequest = null;
  runtime.registerInMemoryProvider("ollama", (request) => {
    turns += 1;
    if (turns === 1) {
      assert.ok(request.tools.some((tool) => tool.name === NEUROSURGERY_GROUNDED_REAL_RESEARCH_BRIEF_TOOL));
      return { toolCalls: [{ id: "brief-view", name: NEUROSURGERY_GROUNDED_REAL_RESEARCH_BRIEF_TOOL,
        arguments: { max_topics: 4, max_records_per_topic: 2 } }] };
    }
    continuationRequest = request;
    return { structured: { answer: "The deterministic topic lane found one exact source row.", unknowns: [], claims: [{
      claim_id: "brief-record", kind: "source_observation", scope: "public_record_metadata",
      text: "The topic lane returned one literature metadata row.",
      citations: [{ record_kind: "literature_article", record_id: "12345678" }],
    }] } };
  });
  const result = await agent.groundedRealDataResearch(
    "Map glioma molecular identity topics.",
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    runtime, "ollama", "llama3.1", { approveProviderCall: true, toolLoop: true, realDataQuery: { limit: 2 } },
  );
  const briefCall = client.calls.find((call) => call.name === NEUROSURGERY_RESEARCH_BRIEF_TOOL);
  assert.equal(briefCall.args.query.max_topics, 4);
  assert.equal(briefCall.args.query.include_abstracts, false);
  const toolMessage = continuationRequest.messages.find((message) => message.role === "tool");
  const payload = JSON.parse(toolMessage.content);
  assert.equal(payload.view, "topic_brief");
  assert.equal(payload.returned_topics, 1);
  assert.equal(payload.topics[0].records[0].record_id, "12345678");
  assert.equal(payload.topics[0].records[0].abstract_excerpt, undefined);
  assert.deepEqual(payload.topics[0].records[0].publication_types, ["Review"]);
  assert.equal(payload.unknowns[0].code, "topic_unknown");
  assert.equal(result.tool_trace[0].summary_digest, "q".repeat(64));
});

test("grounded real-data tool loop exposes cohort landscape with exact project citations", async () => {
  const client = fakeClient();
  const originalCallTool = client.callTool;
  client.callTool = async (name, args = {}) => {
    if (name === NEUROSURGERY_REAL_DATA_COHORT_LANDSCAPE_TOOL) {
      client.calls.push({ name, args });
      return response(name, {
        schema_version: "bioprism-neurosurgery-real-data-cohort-landscape/0.1",
        landscape_digest: "l".repeat(64), bundle_digest: "b".repeat(64), generated_at: "2026-08-31T00:00:00Z",
        query: args.query, total_matching_projects: 2, returned_project_count: 1, omitted_project_count: 1,
        truncated: true,
        project_rows: [{ project_id: "TCGA-GBM", source_id: "gdc_tcga_gbm",
          source_uri: "https://api.gdc.cancer.gov/projects/TCGA-GBM?format=json", name: "Glioblastoma Multiforme",
          primary_site: ["Brain"], disease_types: ["Gliomas"], case_count: 617,
          data_type_metadata_present: true, data_type_counts: [{ data_type: "Aligned Reads", file_count: 3251 }],
          total_file_count: 3251 }],
        total_released_case_inventory: 617,
        data_type_coverage: [{ data_type: "Aligned Reads", project_count: 1, total_file_count: 3251 }],
        shared_data_type_count: 0, shared_data_types: [], projects_with_data_type_metadata: 1,
        projects_without_data_type_metadata: 0, source_ids: ["gdc_tcga_gbm"],
        review_reasons: [{ code: "project_limit", count: 1, detail: "one project omitted by bound" }],
        provenance_bound: true, synthetic_data: false, human_review_required: true, provider: "none", network: false,
        effect: "read_only", limitations: ["aggregate metadata only"],
      });
    }
    return originalCallTool(name, args);
  };
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let turns = 0;
  let continuationRequest = null;
  runtime.registerInMemoryProvider("ollama", (request) => {
    turns += 1;
    if (turns === 1) {
      assert.ok(request.tools.some((tool) => tool.name === NEUROSURGERY_GROUNDED_REAL_COHORT_LANDSCAPE_TOOL));
      return { toolCalls: [{ id: "cohort-view", name: NEUROSURGERY_GROUNDED_REAL_COHORT_LANDSCAPE_TOOL,
        arguments: { max_projects: 1 } }] };
    }
    continuationRequest = request;
    return { structured: { answer: "The bounded cohort view exposes one source-linked TCGA project row.", unknowns: [], claims: [{
      claim_id: "cohort-project", kind: "source_observation", scope: "public_record_metadata",
      text: "The cohort landscape includes a TCGA-GBM project metadata row.",
      citations: [{ record_kind: "genomic_project", record_id: "TCGA-GBM" }],
    }] } };
  });
  const result = await agent.groundedRealDataResearch(
    "Compare public glioma genomic projects.",
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    runtime, "ollama", "llama3.1", { approveProviderCall: true, toolLoop: true,
      realDataQuery: { record_kind: "genomic_project", genomic_data_type: "Aligned Reads", limit: 2 } },
  );
  const cohortCall = client.calls.find((call) => call.name === NEUROSURGERY_REAL_DATA_COHORT_LANDSCAPE_TOOL);
  assert.equal(cohortCall.args.query.max_projects, 1);
  assert.deepEqual(cohortCall.args.query.query, { record_kind: "genomic_project", genomic_data_type: "Aligned Reads", limit: 2 });
  const toolMessage = continuationRequest.messages.find((message) => message.role === "tool");
  const payload = JSON.parse(toolMessage.content);
  assert.equal(payload.view, "cohort_landscape");
  assert.equal(payload.project_rows[0].project_id, "TCGA-GBM");
  assert.equal(payload.project_rows[0].case_count, 617);
  assert.equal(payload.total_released_case_inventory, 617);
  assert.equal(result.tool_trace[0].view, "cohort_landscape");
  assert.equal(result.tool_trace[0].summary_digest, "l".repeat(64));
  assert.deepEqual(result.tool_trace[0].citations, [{ record_kind: "genomic_project", record_id: "TCGA-GBM" }]);
});

test("grounded real-data tool loop supports structured facets without widening scope", async () => {
  const client = fakeClient();
  const originalCallTool = client.callTool;
  client.callTool = async (name, args = {}) => {
    if (name === NEUROSURGERY_REAL_DATA_QUERY_TOOL) {
      client.calls.push({ name, args });
      return response(name, {
        schema_version: "bioprism-neurosurgery-real/0.1",
        query: args.query,
        total_matches: 1,
        returned_matches: 1,
        truncated: false,
        hits: [{
          record_kind: "clinical_trial", record_id: "FACET-TRIAL", title: "Interventional trial",
          source_id: "clinicaltrials_glioma", source_uri: "https://clinicaltrials.gov/study/FACET-TRIAL",
          status: "RECRUITING", phases: ["PHASE2"], study_type: "Interventional", enrollment_count: 42,
          intervention_names: ["metadata-only intervention label"], abstract_excerpt: "A bounded abstract excerpt.",
          related_records: [
            { record_kind: "portal_study", record_id: "GBM-STUDY", relation: "describes_study" },
            { record_kind: "patient_case", record_id: "SHOULD-DROP", relation: "has_profile" },
            { record_kind: "portal_study", record_id: "SHOULD-DROP", relation: "unsupported" },
          ],
        }],
      });
    }
    return originalCallTool(name, args);
  };
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let turns = 0;
  let continuationRequest = null;
  runtime.registerInMemoryProvider("ollama", (request) => {
    turns += 1;
    if (turns === 1) {
      assert.equal(request.tools[0].parameters.properties.trial_study_type.type, "string");
      assert.ok(request.tools[0].parameters.properties.record_kind.enum.includes("clinical_trial"));
      return { toolCalls: [{ id: "facet-search", name: NEUROSURGERY_GROUNDED_REAL_DATA_PROVIDER_TOOL,
        arguments: { record_kind: "clinical_trial", trial_study_type: "Interventional", limit: 128 } }] };
    }
    continuationRequest = request;
    return { structured: {
      answer: "The structured search returned one trial row.", unknowns: [], claims: [{
        claim_id: "facet-trial", kind: "source_observation", scope: "public_record_metadata",
        text: "A bounded structured query returned a clinical-trial metadata row.",
        citations: [{ record_kind: "clinical_trial", record_id: "FACET-TRIAL" }],
      }],
    } };
  });
  const result = await agent.groundedRealDataResearch(
    "Find recruiting trials.",
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    runtime, "ollama", "llama3.1",
    { approveProviderCall: true, toolLoop: true, realDataQuery: { record_kind: "clinical_trial", limit: 1 } },
  );
  const queryCall = client.calls.find((call) => call.name === NEUROSURGERY_REAL_DATA_QUERY_TOOL);
  assert.equal(queryCall.args.query.record_kind, "clinical_trial");
  assert.equal(queryCall.args.query.trial_study_type, "Interventional");
  assert.equal(queryCall.args.query.limit, 1);
  const toolMessage = continuationRequest.messages.find((message) => message.role === "tool");
  const toolPayload = JSON.parse(toolMessage.content);
  assert.equal(toolPayload.hits[0].status, "RECRUITING");
  assert.deepEqual(toolPayload.hits[0].phases, ["PHASE2"]);
  assert.equal(toolPayload.hits[0].enrollment_count, 42);
  assert.equal(toolPayload.hits[0].abstract_excerpt, "A bounded abstract excerpt.");
  assert.deepEqual(toolPayload.hits[0].related_records, [{ record_kind: "portal_study", record_id: "GBM-STUDY", relation: "describes_study" }]);
  assert.equal(result.tool_trace[0].query.trial_study_type, "Interventional");
  assert.equal(result.tool_trace[0].query.text, undefined);
});

test("grounded real-data tool loop exposes trial-landscape view with citations", async () => {
  const client = fakeClient();
  const originalCallTool = client.callTool;
  client.callTool = async (name, args = {}) => {
    if (name === NEUROSURGERY_REAL_DATA_QUERY_TOOL) {
      client.calls.push({ name, args });
      return response(name, {
        schema_version: "bioprism-neurosurgery-real/0.1", query: args.query,
        total_matches: 1, returned_matches: 1, truncated: false,
        hits: [{ record_kind: "clinical_trial", record_id: "VIEW-TRIAL", title: "Recruiting glioma trial",
          source_id: "clinicaltrials_glioma", source_uri: "https://clinicaltrials.gov/study/VIEW-TRIAL",
          status: "RECRUITING", phases: ["PHASE2"], study_type: "Interventional", enrollment_count: 37,
          intervention_names: ["metadata-only label"] }],
      });
    }
    return originalCallTool(name, args);
  };
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let turns = 0;
  let continuationRequest = null;
  runtime.registerInMemoryProvider("ollama", (request) => {
    turns += 1;
    if (turns === 1) {
      const names = request.tools.map((tool) => tool.name);
      assert.ok(names.includes(NEUROSURGERY_GROUNDED_REAL_TRIAL_LANDSCAPE_TOOL));
      assert.ok(names.includes(NEUROSURGERY_GROUNDED_REAL_MOLECULAR_COVERAGE_TOOL));
      return { toolCalls: [{ id: "trial-view", name: NEUROSURGERY_GROUNDED_REAL_TRIAL_LANDSCAPE_TOOL,
        arguments: { trial_study_type: "Interventional", limit: 128, max_interventions: 4 } }] };
    }
    continuationRequest = request;
    return { structured: { answer: "The bounded registry view found one trial row.", unknowns: [], claims: [{
      claim_id: "view-trial", kind: "source_observation", scope: "public_record_metadata",
      text: "The trial-landscape view returned one recruiting metadata row.",
      citations: [{ record_kind: "clinical_trial", record_id: "VIEW-TRIAL" }],
    }] } };
  });
  const result = await agent.groundedRealDataResearch(
    "Reconnoiter recruiting glioma trials.", { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    runtime, "ollama", "llama3.1", { approveProviderCall: true, toolLoop: true,
      realDataQuery: { record_kind: "clinical_trial", limit: 1 } },
  );
  const queryCall = client.calls.find((call) => call.name === NEUROSURGERY_REAL_DATA_QUERY_TOOL);
  assert.equal(queryCall.args.query.record_kind, "clinical_trial");
  assert.equal(queryCall.args.query.trial_study_type, "Interventional");
  assert.equal(queryCall.args.query.limit, 1);
  const toolMessage = continuationRequest.messages.find((message) => message.role === "tool");
  const toolPayload = JSON.parse(toolMessage.content);
  assert.equal(toolPayload.view, "trial_landscape");
  assert.equal(toolPayload.summary.total_matching_trials, 2);
  assert.equal(toolPayload.summary.synthetic_data, false);
  assert.equal(toolPayload.hits[0].record_id, "VIEW-TRIAL");
  assert.equal(result.tool_trace[0].summary_digest, "l".repeat(64));
});

test("grounded real-data tool loop exposes molecular-coverage view without patient values", async () => {
  const client = fakeClient();
  const originalCallTool = client.callTool;
  client.callTool = async (name, args = {}) => {
    if (name === NEUROSURGERY_REAL_DATA_QUERY_TOOL) {
      client.calls.push({ name, args });
      return response(name, {
        schema_version: "bioprism-neurosurgery-real/0.1", query: args.query,
        total_matches: 1, returned_matches: 1, truncated: false,
        hits: [{ record_kind: "portal_molecular_profile", record_id: "VIEW-PROFILE", title: "GBM mutation profile",
          source_id: "cbioportal_gbm_catalog", source_uri: "https://www.cbioportal.org/",
          molecular_alteration_type: "MUTATION_EXTENDED", datatype: "MAF", molecular_description: "public assay metadata",
          molecular_patient_level: true }],
      });
    }
    return originalCallTool(name, args);
  };
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let turns = 0;
  let continuationRequest = null;
  runtime.registerInMemoryProvider("ollama", (request) => {
    turns += 1;
    if (turns === 1) return { toolCalls: [{ id: "coverage-view", name: NEUROSURGERY_GROUNDED_REAL_MOLECULAR_COVERAGE_TOOL,
      arguments: { molecular_datatype: "MAF", limit: 128, max_studies: 2 } }] };
    continuationRequest = request;
    return { structured: { answer: "The bounded molecular coverage view found one profile metadata row.", unknowns: [], claims: [{
      claim_id: "view-profile", kind: "source_observation", scope: "public_record_metadata",
      text: "The molecular-coverage view returned one public assay metadata row.",
      citations: [{ record_kind: "portal_molecular_profile", record_id: "VIEW-PROFILE" }],
    }] } };
  });
  const result = await agent.groundedRealDataResearch(
    "Inventory MAF coverage for glioma.", { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    runtime, "ollama", "llama3.1", { approveProviderCall: true, toolLoop: true,
      realDataQuery: { record_kind: "portal_molecular_profile", limit: 1 } },
  );
  const toolMessage = continuationRequest.messages.find((message) => message.role === "tool");
  const toolPayload = JSON.parse(toolMessage.content);
  assert.equal(toolPayload.view, "molecular_coverage");
  assert.equal(toolPayload.summary.total_matching_profile_count, 6);
  assert.equal(toolPayload.summary.synthetic_data, false);
  assert.equal(toolPayload.hits[0].record_id, "VIEW-PROFILE");
  assert.equal(toolPayload.hits[0].molecular_patient_level, true);
  assert.equal(result.tool_trace[0].summary_digest, "m".repeat(64));
});

test("grounded real-data tool loop exposes identifier-reconciliation view", async () => {
  const client = fakeClient();
  const originalCallTool = client.callTool;
  client.callTool = async (name, args = {}) => {
    if (name === NEUROSURGERY_REAL_DATA_RECONCILIATION_TOOL) {
      client.calls.push({ name, args });
      return response(name, {
        schema_version: "bioprism-neurosurgery-real-data-reconciliation/0.1", reconciliation_digest: "r".repeat(64),
        bundle_digest: "b".repeat(64), generated_at: "2026-08-30T00:00:00Z", query: args.query,
        counts: {
          portal_study_count: 1, portal_study_with_pmid_count: 1, portal_study_without_pmid_count: 0,
          portal_pmid_missing_literature_count: 1, shared_portal_pmid_count: 0, literature_article_count: 1,
          literature_with_doi_count: 1, shared_literature_doi_count: 0,
        },
        candidate_issue_count: 1, returned_issue_count: 1, omitted_issue_count: 0, truncated: false,
        issues: [{ kind: "portal_pmid_missing_literature", identifier: "99999999", record_kind: "portal_study",
          record_id: "gbm_tcga_pub", source_id: "cbioportal_gbm_catalog", related_record_ids: [],
          detail: "The portal PMID is not present in the literature snapshot.", patient_values: ["must-drop"] }],
        requires_review: true, provenance_bound: true, synthetic_data: false, human_review_required: true,
        provider: "none", network: false, effect: "read_only", limitations: ["metadata-only identifier crosswalk"],
      });
    }
    return originalCallTool(name, args);
  };
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let turns = 0;
  let continuationRequest = null;
  runtime.registerInMemoryProvider("ollama", (request) => {
    turns += 1;
    if (turns === 1) {
      assert.ok(request.tools.map((tool) => tool.name).includes(NEUROSURGERY_GROUNDED_REAL_RECONCILIATION_TOOL));
      return { toolCalls: [{ id: "reconcile-view", name: NEUROSURGERY_GROUNDED_REAL_RECONCILIATION_TOOL, arguments: { max_issues: 8 } }] };
    }
    continuationRequest = request;
    return { structured: { answer: "The bundle has one unresolved identifier crosswalk obligation.", unknowns: [], claims: [{
      claim_id: "reconcile-study", kind: "source_observation", scope: "public_record_metadata",
      text: "One portal PMID is not represented in the literature snapshot.",
      citations: [{ record_kind: "portal_study", record_id: "gbm_tcga_pub" }],
    }] } };
  });
  const result = await agent.groundedRealDataResearch(
    "Audit glioma identifier crosswalks.", { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    runtime, "ollama", "llama3.1", { approveProviderCall: true, toolLoop: true, realDataQuery: { limit: 2 } },
  );
  const reconciliationCall = client.calls.find((call) => call.name === NEUROSURGERY_REAL_DATA_RECONCILIATION_TOOL);
  assert.deepEqual(reconciliationCall.args.query, { max_issues: 2 });
  const toolMessage = continuationRequest.messages.find((message) => message.role === "tool");
  const toolPayload = JSON.parse(toolMessage.content);
  assert.equal(toolPayload.view, "identifier_reconciliation");
  assert.equal(toolPayload.returned_issues, 1);
  assert.equal(toolPayload.issues[0].record_id, "gbm_tcga_pub");
  assert.equal(toolPayload.issues[0].patient_values, undefined);
  assert.equal(toolPayload.summary.reconciliation_digest, "r".repeat(64));
  assert.equal(result.tool_trace[0].summary_digest, "r".repeat(64));
});

test("grounded real-data tool loop exposes review-queue items with citations", async () => {
  const client = fakeClient();
  const originalCallTool = client.callTool;
  client.callTool = async (name, args = {}) => {
    if (name === NEUROSURGERY_REAL_DATA_REVIEW_QUEUE_TOOL) {
      client.calls.push({ name, args });
      return response(name, {
        schema_version: "bioprism-neurosurgery-real-data-review-queue/0.1", bundle_digest: "b".repeat(64),
        queue_digest: "q".repeat(64), generated_at: "2026-08-30T00:00:00Z", query: args.query,
        source_count: 5, record_count: 88, candidate_item_count: 2, returned_item_count: 1,
        omitted_item_count: 1, truncated: true,
        items: [{ task_id: "review-portal-1", class: "provenance", kind: "missing_portal_publication_link",
          status: "needs_human_review", source_id: "cbioportal_gbm_catalog", source_kind: "study_portal",
          source_uri: "https://www.cbioportal.org/", record_kind: "portal_study", record_id: "QUEUE-STUDY",
          title: "Public glioma study", reason: "Verify whether a publication crosswalk exists.",
          reviewer_roles: ["neuro-oncology"], patient_values: ["must-drop"] }],
        provenance_bound: true, synthetic_data: false, human_review_required: true, provider: "none",
        network: false, effect: "read_only", limitations: ["metadata-only queue"],
      });
    }
    return originalCallTool(name, args);
  };
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let turns = 0;
  let continuationRequest = null;
  runtime.registerInMemoryProvider("ollama", (request) => {
    turns += 1;
    if (turns === 1) {
      assert.ok(request.tools.map((tool) => tool.name).includes(NEUROSURGERY_GROUNDED_REAL_REVIEW_QUEUE_TOOL));
      return { toolCalls: [{ id: "queue-view", name: NEUROSURGERY_GROUNDED_REAL_REVIEW_QUEUE_TOOL,
        arguments: { record_kind: "portal_study", max_items: 128 } }] };
    }
    continuationRequest = request;
    return { structured: { answer: "The snapshot has one unresolved public-study provenance task.", unknowns: [], claims: [{
      claim_id: "queue-study", kind: "source_observation", scope: "public_record_metadata",
      text: "A public-study publication crosswalk remains explicitly unresolved.",
      citations: [{ record_kind: "portal_study", record_id: "QUEUE-STUDY" }],
    }] } };
  });
  const result = await agent.groundedRealDataResearch(
    "Find unresolved glioma provenance obligations.",
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    runtime, "ollama", "llama3.1", { approveProviderCall: true, toolLoop: true,
      realDataQuery: { record_kind: "portal_study", limit: 2 } },
  );
  const queueCall = client.calls.find((call) => call.name === NEUROSURGERY_REAL_DATA_REVIEW_QUEUE_TOOL);
  assert.deepEqual(queueCall.args.query, { record_kind: "portal_study", max_items: 2 });
  const toolMessage = continuationRequest.messages.find((message) => message.role === "tool");
  const toolPayload = JSON.parse(toolMessage.content);
  assert.equal(toolPayload.view, "review_queue");
  assert.equal(toolPayload.returned_items, 1);
  assert.equal(toolPayload.items[0].patient_values, undefined);
  assert.equal(toolPayload.summary.queue_digest, "q".repeat(64));
  assert.equal(result.tool_trace[0].summary_digest, "q".repeat(64));
  assert.equal(result.audit.status, "grounded_for_human_review");
});

test("grounded real-data tool loop exposes the evidence graph crosswalk", async () => {
  const client = fakeClient();
  const originalCallTool = client.callTool;
  client.callTool = async (name, args = {}) => {
    if (name === NEUROSURGERY_EVIDENCE_GRAPH_TOOL) {
      client.calls.push({ name, args });
      return response(name, {
        schema_version: "bioprism-neurosurgery-evidence-graph/0.1",
        bundle_digest: "b".repeat(64), graph_digest: "g".repeat(64), specialty: "glioma", query: args.query,
        nodes: [
          { record_kind: "genomic_project", record_id: "TCGA-GBM", title: "TCGA glioblastoma project", source_id: "gdc", source_uri: "https://portal.gdc.cancer.gov/projects/TCGA-GBM" },
          { record_kind: "literature_article", record_id: "GRAPH-PMID", title: "Linked glioma citation", source_id: "pubmed", source_uri: "https://pubmed.ncbi.nlm.nih.gov/GRAPH-PMID/" },
        ],
        edges: [{ from_record_kind: "genomic_project", from_record_id: "TCGA-GBM", to_record_kind: "literature_article", to_record_id: "GRAPH-PMID", relation: "published_as" }],
        total_node_count: 2, total_edge_count: 1, omitted_node_count: 0, omitted_edge_count: 0, truncated: false,
        root_count: 1, connected_component_count: 1, isolated_node_count: 0, source_count: 2, bundle_relationship_count: 1,
        human_review_required: true, provider: "none", network: false, effect: "read_only", limitations: ["explicit identifier crosswalk only"],
      });
    }
    return originalCallTool(name, args);
  };
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let turns = 0;
  runtime.registerInMemoryProvider("ollama", (request) => {
    turns += 1;
    if (turns === 1) {
      assert.equal(request.tools.some((tool) => tool.name === NEUROSURGERY_GROUNDED_REAL_EVIDENCE_GRAPH_TOOL), true);
      return { toolCalls: [{ id: "graph-view", name: NEUROSURGERY_GROUNDED_REAL_EVIDENCE_GRAPH_TOOL,
        arguments: { root_record_id: "TCGA-GBM", root_record_kind: "genomic_project", max_nodes: 128, max_edges: 256 } }] };
    }
    return { structured: {
      answer: "The source crosswalk links a public project to a PMID.", unknowns: [], claims: [{
        claim_id: "graph-link", kind: "source_observation", scope: "public_record_metadata",
        text: "The bounded graph contains an explicit project-to-literature identifier edge.",
        citations: [{ record_kind: "literature_article", record_id: "GRAPH-PMID" }],
      }],
    } };
  });
  const result = await agent.groundedRealDataResearch(
    "Inspect glioma source crosswalks.",
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    runtime, "ollama", "llama3.1", { approveProviderCall: true, toolLoop: true, realDataQuery: { limit: 2 } },
  );
  const graphCall = client.calls.find((call) => call.name === NEUROSURGERY_EVIDENCE_GRAPH_TOOL);
  assert.deepEqual(graphCall.args.query, { root_record_id: "TCGA-GBM", root_record_kind: "genomic_project", max_nodes: 2, max_edges: 4 });
  assert.equal(client.calls.some((call) => call.name === NEUROSURGERY_REAL_DATA_QUERY_TOOL), false);
  assert.equal(result.tool_trace[0].summary_digest, "g".repeat(64));
  assert.equal(result.tool_trace[0].summary.returned_node_count, 2);
  assert.equal(result.claims[0].citations[0].record_id, "GRAPH-PMID");
});

test("grounded real-data tool loop exposes the next evidence worklist", async () => {
  const client = fakeClient();
  const originalCallTool = client.callTool;
  client.callTool = async (name, args = {}) => {
    if (name === NEUROSURGERY_EVIDENCE_ACQUISITION_TOOL) {
      client.calls.push({ name, args });
      return response(name, {
        schema_version: "bioprism-neurosurgery-evidence-acquisition/0.1",
        plan_digest: "a".repeat(64), request_digest: "r".repeat(64), specialty: "glioma", query: args.query,
        audit: {},
        steps: [{
          sequence: 1, step_id: "step-1", source: "real_glioma_population", trigger: "missing_evidence_record",
          query: { source: "real_glioma_population", query: { record_kind: "clinical_trial", limit: 2 } },
          fallback_to_specialty_scan: false, status: "candidates_found", total_matches: 2, returned_matches: 2, truncated: false,
          references: [{ source: "real_glioma_population", source_id: "clinicaltrials_glioma", record_id: "NCT00000001", title: "Bounded trial metadata", uri: "https://clinicaltrials.gov/study/NCT00000001" }],
        }],
        candidate_step_count: 1, omitted_step_count: 0, truncated: false, source_query_count: 1, source_candidate_count: 2,
        required_sources: [], ready_for_local_replay: true, human_review_required: true, provider: "none", network: false,
        effect: "read_only", limitations: ["local query worklist only"],
      });
    }
    return originalCallTool(name, args);
  };
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let turns = 0;
  runtime.registerInMemoryProvider("ollama", (request) => {
    turns += 1;
    if (turns === 1) {
      assert.equal(request.tools.some((tool) => tool.name === NEUROSURGERY_GROUNDED_REAL_EVIDENCE_ACQUISITION_TOOL), true);
      return { toolCalls: [{ id: "acquisition-view", name: NEUROSURGERY_GROUNDED_REAL_EVIDENCE_ACQUISITION_TOOL,
        arguments: { max_steps: 64, max_references_per_step: 16 } }] };
    }
    return { structured: {
      answer: "The bounded worker found a trial metadata query for reviewer replay.", unknowns: [], claims: [{
        claim_id: "acquisition-plan", kind: "limitation", scope: "public_record_metadata",
        text: "The next-evidence plan is a reviewer-owned local query, not a clinical finding.",
        citations: [{ record_kind: "clinical_trial", record_id: "NCT00000001" }],
      }],
    } };
  });
  const result = await agent.groundedRealDataResearch(
    "Find the next bounded glioma evidence wave.",
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    runtime, "ollama", "llama3.1", { approveProviderCall: true, toolLoop: true, realDataQuery: { limit: 2 } },
  );
  const acquisitionCall = client.calls.find((call) => call.name === NEUROSURGERY_EVIDENCE_ACQUISITION_TOOL);
  assert.deepEqual(acquisitionCall.args.query, { max_steps: 2, max_references_per_step: 16 });
  assert.equal(result.tool_trace[0].summary_digest, "a".repeat(64));
  assert.equal(result.tool_trace[0].summary.returned_step_count, 1);
  assert.equal(result.audit.status, "grounded_for_human_review");
});

test("grounded real-data tool loop exposes the specialist evidence map", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let turns = 0;
  runtime.registerInMemoryProvider("ollama", (request) => {
    turns += 1;
    if (turns === 1) {
      assert.equal(request.tools.some((tool) => tool.name === NEUROSURGERY_GROUNDED_SPECIALTY_EVIDENCE_MAP_TOOL), true);
      return { toolCalls: [{ id: "specialty-map", name: NEUROSURGERY_GROUNDED_SPECIALTY_EVIDENCE_MAP_TOOL, arguments: { max_dimensions: 32 } }] };
    }
    return { structured: {
      answer: "The specialist map reports an explicit coverage hold for human review.", unknowns: [], claims: [{
        claim_id: "map-hold", kind: "limitation", scope: "public_record_metadata",
        text: "The specialist map is a coverage ledger and does not establish a patient finding.",
        citations: [{ record_kind: "genomic_project", record_id: "TCGA-GBM" }],
      }],
    } };
  });
  const result = await agent.groundedRealDataResearch(
    "Map the specialist glioma evidence coverage.",
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    runtime, "ollama", "llama3.1", { approveProviderCall: true, toolLoop: true, realDataQuery: { limit: 2 } },
  );
  const mapCall = client.calls.find((call) => call.name === NEUROSURGERY_SPECIALTY_EVIDENCE_MAP_TOOL);
  assert.equal(mapCall.args.request.specialty, "glioma");
  assert.equal(result.tool_trace[0].view, "specialty_evidence_map");
  assert.equal(result.tool_trace[0].map_digest, "m".repeat(64));
  assert.equal(result.tool_trace[0].returned_dimensions, 1);
  assert.equal(result.tool_trace[0].summary.dimensions[0].key, "tumor_identity");
  assert.equal(result.claims[0].citations[0].record_id, "TCGA-GBM");
});

test("grounded real-data tool loop exposes caller-clocked freshness", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let turns = 0;
  runtime.registerInMemoryProvider("ollama", (request) => {
    turns += 1;
    if (turns === 1) {
      assert.equal(request.tools.some((tool) => tool.name === NEUROSURGERY_GROUNDED_REAL_FRESHNESS_TOOL), true);
      return { toolCalls: [{ id: "freshness-view", name: NEUROSURGERY_GROUNDED_REAL_FRESHNESS_TOOL, arguments: { max_sources: 8 } }] };
    }
    return { structured: {
      answer: "The caller-clocked snapshot is stale and requires human review.", unknowns: [], claims: [{
        claim_id: "freshness-hold", kind: "limitation", scope: "public_record_metadata",
        text: "Source age is a metadata hold and does not establish a clinical finding.",
        citations: [{ record_kind: "genomic_project", record_id: "TCGA-GBM" }],
      }],
    } };
  });
  const result = await agent.groundedRealDataResearch(
    "Check the freshness of the glioma snapshot.",
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    runtime, "ollama", "llama3.1",
    { approveProviderCall: true, toolLoop: true, freshness: { as_of: "2026-08-31T00:00:00Z", max_age_days: 180 }, realDataQuery: { limit: 1 } },
  );
  const freshnessCall = client.calls.find((call) => call.name === NEUROSURGERY_REAL_DATA_FRESHNESS_TOOL);
  assert.deepEqual(freshnessCall.args.query, { as_of: "2026-08-31T00:00:00Z", max_age_days: 180 });
  assert.equal(result.tool_trace[0].view, "freshness");
  assert.equal(result.tool_trace[0].freshness_digest, "f".repeat(64));
  assert.equal(result.tool_trace[0].freshness_status, "stale");
});

test("grounded real-data freshness requires an explicit caller clock", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let turns = 0;
  runtime.registerInMemoryProvider("ollama", () => {
    turns += 1;
    if (turns === 1) return { toolCalls: [{ id: "freshness-no-clock", name: NEUROSURGERY_GROUNDED_REAL_FRESHNESS_TOOL, arguments: {} }] };
    return { structured: { answer: "The freshness request was held for a caller clock.", unknowns: [], claims: [{
      claim_id: "freshness-clock", kind: "limitation", scope: "public_record_metadata",
      text: "Freshness cannot be evaluated without an explicit UTC caller clock.",
      citations: [{ record_kind: "genomic_project", record_id: "TCGA-GBM" }],
    }] } };
  });
  const result = await agent.groundedRealDataResearch(
    "Check the freshness of the glioma snapshot.",
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    runtime, "ollama", "llama3.1", { approveProviderCall: true, toolLoop: true, realDataQuery: { limit: 1 } },
  );
  assert.equal(result.tool_trace[0].status, "error");
  assert.match(result.tool_trace[0].error, /explicit caller freshness clock/);
});

test("grounded real-data tool loop exposes snapshot coverage", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let turns = 0;
  runtime.registerInMemoryProvider("ollama", (request) => {
    turns += 1;
    if (turns === 1) {
      assert.equal(request.tools.some((tool) => tool.name === NEUROSURGERY_GROUNDED_REAL_COVERAGE_TOOL), true);
      return { toolCalls: [{ id: "coverage-view", name: NEUROSURGERY_GROUNDED_REAL_COVERAGE_TOOL, arguments: { record_kind: "literature_article" } }] };
    }
    return { structured: {
      answer: "The snapshot coverage is a bounded metadata inventory with an explicit gap.", unknowns: [], claims: [{
        claim_id: "coverage-hold", kind: "limitation", scope: "public_record_metadata",
        text: "Coverage gaps remain reviewer-owned metadata obligations and do not establish a clinical finding.",
        citations: [{ record_kind: "genomic_project", record_id: "TCGA-GBM" }],
      }],
    } };
  });
  const result = await agent.groundedRealDataResearch(
    "Audit source and temporal coverage of the glioma snapshot.",
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    runtime, "ollama", "llama3.1", { approveProviderCall: true, toolLoop: true, realDataQuery: { limit: 2 } },
  );
  const coverageCall = client.calls.find((call) => call.name === NEUROSURGERY_REAL_DATA_COVERAGE_TOOL);
  assert.deepEqual(coverageCall.args.query, { record_kind: "literature_article" });
  assert.equal(result.tool_trace[0].view, "coverage");
  assert.equal(result.tool_trace[0].coverage_digest, "c".repeat(64));
  assert.equal(result.tool_trace[0].returned_sources, 1);
  assert.equal(result.tool_trace[0].returned_gaps, 1);
});

test("grounded real-data review queue rejects unrepresentable caller facets", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let turns = 0;
  runtime.registerInMemoryProvider("ollama", () => {
    turns += 1;
    if (turns === 1) return { toolCalls: [{ id: "queue-bad", name: NEUROSURGERY_GROUNDED_REAL_REVIEW_QUEUE_TOOL, arguments: { max_items: 8 } }] };
    return { structured: { answer: "The caller-bound query remains available for review.", unknowns: [], claims: [{
      claim_id: "queue-bound", kind: "source_observation", scope: "public_record_metadata",
      text: "The caller supplied a structured trial facet.", citations: [{ record_kind: "clinical_trial", record_id: "NCT00000001" }],
    }] } };
  });
  const result = await agent.groundedRealDataResearch(
    "Review trial metadata obligations.",
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    runtime, "ollama", "llama3.1", { approveProviderCall: true, toolLoop: true,
      realDataQuery: { record_kind: "clinical_trial", trial_phase: "PHASE2", limit: 1 } },
  );
  assert.equal(result.tool_trace[0].status, "error");
  assert.match(result.tool_trace[0].error, /cannot combine caller facet trial_phase/);
  assert.deepEqual(client.calls.map((call) => call.name), [
    NEUROSURGERY_REAL_DATA_REASONING_CONTEXT_TOOL, NEUROSURGERY_REAL_DATA_DRAFT_AUDIT_TOOL,
  ]);
});

test("grounded real-data tool loop rejects structured facet overrides", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let turns = 0;
  runtime.registerInMemoryProvider("ollama", () => {
    turns += 1;
    if (turns === 1) {
      return { toolCalls: [{ id: "bad-facet", name: NEUROSURGERY_GROUNDED_REAL_DATA_PROVIDER_TOOL,
        arguments: { text: "genomic", record_kind: "genomic_project" } }] };
    }
    return { structured: {
      answer: "The caller-bound trial context remains available.", unknowns: [], claims: [{
        claim_id: "caller-bound", kind: "source_observation", scope: "public_record_metadata",
        text: "The caller supplied a clinical-trial lane.",
        citations: [{ record_kind: "clinical_trial", record_id: "NCT00000001" }],
      }],
    } };
  });
  const result = await agent.groundedRealDataResearch(
    "Find trial metadata.",
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    runtime, "ollama", "llama3.1",
    { approveProviderCall: true, toolLoop: true, realDataQuery: { record_kind: "clinical_trial", limit: 1 } },
  );
  assert.equal(result.tool_trace[0].status, "error");
  assert.match(result.tool_trace[0].error, /cannot override caller facet record_kind/);
  assert.deepEqual(client.calls.map((call) => call.name), [
    NEUROSURGERY_REAL_DATA_REASONING_CONTEXT_TOOL,
    NEUROSURGERY_REAL_DATA_DRAFT_AUDIT_TOOL,
  ]);
});

test("grounded real-data bridge rejects citations omitted from the model context", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  runtime.registerInMemoryProvider("ollama", () => ({
    structured: {
      answer: "The answer cites a record that was not supplied.",
      unknowns: [],
      claims: [{
        claim_id: "out-of-context",
        kind: "source_observation",
        scope: "public_record_metadata",
        text: "This source identity was not present in the bounded context.",
        citations: [{ record_kind: "guideline_reference", record_id: "hidden-guideline" }],
      }],
    },
  }));
  await assert.rejects(
    () => agent.groundedRealDataResearch(
      "Summarize the bounded glioma metadata.",
      { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
      runtime,
      "ollama",
      "llama3.1",
      { approveProviderCall: true, includeAbstracts: false },
    ),
    ProtocolError,
  );
  assert.deepEqual(client.calls.map((call) => call.name), [NEUROSURGERY_REAL_DATA_REASONING_CONTEXT_TOOL]);
});

test("grounded public-literature bridge covers congenital and craniocervical lanes", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  runtime.registerInMemoryProvider("ollama", () => ({
    structured: {
      answer: "The selected lane contains source-linked Chiari literature for reviewer inspection.",
      unknowns: ["The citation set does not establish an individual patient finding."],
      claims: [{
        claim_id: "chiari-literature",
        kind: "source_observation",
        scope: "citation_metadata",
        text: "The bounded PubMed packet contains a Chiari citation.",
        citations: [{ record_kind: "literature_article", record_id: "12345678" }],
      }],
    },
  }));
  const bundle = { schema_version: "bioprism-neurosurgery-public-literature/0.1", synthetic_data: false, sources: [], records: [] };
  const result = await agent.groundedPublicLiteratureResearch(
    "What source-linked Chiari literature is available?", bundle, runtime, "ollama", "llama3.1",
    { specialty: "chiari_malformation", approveProviderCall: true, includeAbstracts: false, freshness: { as_of: "2026-08-31T00:00:00Z", max_age_days: 180 } },
  );
  assert.equal(result.schema_version, "bioprism-neurosurgery-grounded-literature-research/0.1");
  assert.equal(result.specialty, "chiari_malformation");
  assert.equal(result.status, "grounded_for_human_review");
  assert.equal(result.transport, "in_memory");
  assert.equal(result.audit.status, "grounded_for_human_review");
  assert.equal(result.human_review_required, true);
  assert.equal(client.calls.filter((call) => call.name === NEUROSURGERY_PUBLIC_LITERATURE_REASONING_CONTEXT_TOOL).length, 1);
  assert.equal(client.calls.filter((call) => call.name === NEUROSURGERY_PUBLIC_LITERATURE_DRAFT_AUDIT_TOOL).length, 1);
  const contextCall = client.calls.find((call) => call.name === NEUROSURGERY_PUBLIC_LITERATURE_REASONING_CONTEXT_TOOL);
  assert.equal(contextCall.args.query.packet.query.specialty, "chiari_malformation");
  assert.equal(contextCall.args.query.packet.freshness.max_age_days, 180);
  const literatureAuditCall = client.calls.find((call) => call.name === NEUROSURGERY_PUBLIC_LITERATURE_DRAFT_AUDIT_TOOL);
  assert.equal(literatureAuditCall.args.query.freshness.max_age_days, 180);
});

test("grounded public-literature tool loop supports structured facets", async () => {
  const client = fakeClient();
  const originalCallTool = client.callTool;
  client.callTool = async (name, args = {}) => {
    if (name === NEUROSURGERY_PUBLIC_LITERATURE_QUERY_TOOL) {
      client.calls.push({ name, args });
      return response(name, {
        schema_version: "bioprism-neurosurgery-public-literature/0.1",
        query: args.query,
        total_matches: 1,
        returned_matches: 1,
        truncated: false,
        hits: [{ specialty: "chiari_malformation", pmid: "FACET-PMID", title: "Chiari review", journal: "Neurosurgery",
          source_id: "pubmed_chiari", source_uri: "https://pubmed.ncbi.nlm.nih.gov/FACET-PMID/", record_uri: "https://pubmed.ncbi.nlm.nih.gov/FACET-PMID/" }],
        abstract_count: 0, abstract_truncated_count: 0, specialty_counts: [],
      });
    }
    return originalCallTool(name, args);
  };
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let turns = 0;
  runtime.registerInMemoryProvider("ollama", (request) => {
    turns += 1;
    if (turns === 1) {
      assert.equal(request.tools[0].parameters.properties.mesh_term.type, "string");
      assert.equal(request.tools[0].parameters.properties.specialty, undefined);
      return { toolCalls: [{ id: "literature-facet", name: NEUROSURGERY_GROUNDED_LITERATURE_PROVIDER_TOOL,
        arguments: { mesh_term: "Chiari Malformation", limit: 128 } }] };
    }
    return { structured: {
      answer: "The structured literature search returned one citation.", unknowns: [], claims: [{
        claim_id: "facet-pmid", kind: "source_observation", scope: "citation_metadata",
        text: "A bounded structured PubMed query returned one citation.",
        citations: [{ record_kind: "literature_article", record_id: "FACET-PMID" }],
      }],
    } };
  });
  const result = await agent.groundedPublicLiteratureResearch(
    "Find Chiari reviews.",
    { schema_version: "bioprism-neurosurgery-public-literature/0.1", synthetic_data: false },
    runtime, "ollama", "llama3.1",
    { specialty: "chiari_malformation", approveProviderCall: true, toolLoop: true,
      publicLiteratureQuery: { specialty: "chiari_malformation", publication_type: "Review", limit: 1 } },
  );
  const queryCall = client.calls.find((call) => call.name === NEUROSURGERY_PUBLIC_LITERATURE_QUERY_TOOL);
  assert.equal(queryCall.args.query.specialty, "chiari_malformation");
  assert.equal(queryCall.args.query.publication_type, "Review");
  assert.equal(queryCall.args.query.mesh_term, "Chiari Malformation");
  assert.equal(queryCall.args.query.limit, 1);
  assert.equal(result.tool_trace[0].query.mesh_term, "Chiari Malformation");
  assert.equal(result.tool_trace[0].query.text, undefined);
});

test("grounded public-literature tool loop supports the integrity review queue", async () => {
  const client = fakeClient();
  const originalCallTool = client.callTool;
  client.callTool = async (name, args = {}) => {
    if (name === NEUROSURGERY_PUBLIC_LITERATURE_REVIEW_QUEUE_TOOL) {
      client.calls.push({ name, args });
      return response(name, {
        schema_version: "bioprism-neurosurgery-public-literature-review-queue/0.1",
        bundle_digest: "p".repeat(64), queue_digest: "q".repeat(64), integrity_audit_digest: "i".repeat(64),
        generated_at: "2026-08-31T00:00:00Z", query: args.query,
        candidate_item_count: 2, returned_item_count: 1, omitted_item_count: 1, omitted_integrity_issue_count: 0,
        truncated: true,
        items: [{
          task_id: "queue-task-1", class: "completeness", kind: "missing_abstract", status: "needs_human_review",
          specialty: "chiari_malformation", source_id: "pubmed_chiari",
          source_uri: "https://pubmed.ncbi.nlm.nih.gov/QUEUE-PMID/", pmid: "QUEUE-PMID",
          record_uri: "https://pubmed.ncbi.nlm.nih.gov/QUEUE-PMID/", title: "A citation needing abstract review",
          related_pmids: ["12345678"], reason: "abstract is absent from the checked-in snapshot", reviewer_roles: ["neurosurgery"],
          patient_values: { should: "never cross" },
        }],
        provenance_bound: true, synthetic_data: false, human_review_required: true,
        provider: "none", network: false, effect: "read_only", limitations: [],
      });
    }
    return originalCallTool(name, args);
  };
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let turns = 0;
  runtime.registerInMemoryProvider("ollama", (request) => {
    turns += 1;
    if (turns === 1) {
      assert.equal(request.tools[1].name, NEUROSURGERY_GROUNDED_LITERATURE_REVIEW_QUEUE_TOOL);
      return { toolCalls: [{ id: "queue-call", name: NEUROSURGERY_GROUNDED_LITERATURE_REVIEW_QUEUE_TOOL, arguments: { max_items: 128 } }] };
    }
    return { structured: {
      answer: "The corpus queue contains a bounded metadata task.", unknowns: [], claims: [{
        claim_id: "queue-claim", kind: "source_observation", scope: "citation_metadata",
        text: "One PubMed record is flagged for human abstract review.",
        citations: [{ record_kind: "literature_article", record_id: "QUEUE-PMID" }],
      }],
    } };
  });
  const result = await agent.groundedPublicLiteratureResearch(
    "Review Chiari corpus completeness.",
    { schema_version: "bioprism-neurosurgery-public-literature/0.1", synthetic_data: false },
    runtime, "ollama", "llama3.1",
    { specialty: "chiari_malformation", approveProviderCall: true, toolLoop: true,
      publicLiteratureQuery: { specialty: "chiari_malformation", limit: 2 } },
  );
  const queueCall = client.calls.find((call) => call.name === NEUROSURGERY_PUBLIC_LITERATURE_REVIEW_QUEUE_TOOL);
  assert.deepEqual(queueCall.args.query, { specialties: ["chiari_malformation"], max_items: 2 });
  assert.equal(client.calls.some((call) => call.name === NEUROSURGERY_PUBLIC_LITERATURE_QUERY_TOOL), false);
  assert.equal(result.tool_trace[0].view, "review_queue");
  assert.equal(result.tool_trace[0].queue_digest, "q".repeat(64));
  assert.equal(result.claims[0].citations[0].record_id, "QUEUE-PMID");
  assert.equal(result.human_review_required, true);
});

test("grounded public-literature tool loop exposes the next evidence worklist", async () => {
  const client = fakeClient();
  const originalCallTool = client.callTool;
  client.callTool = async (name, args = {}) => {
    if (name === NEUROSURGERY_EVIDENCE_ACQUISITION_TOOL) {
      client.calls.push({ name, args });
      return response(name, {
        schema_version: "bioprism-neurosurgery-evidence-acquisition/0.1",
        plan_digest: "l".repeat(64), request_digest: "r".repeat(64), specialty: "chiari_malformation", query: args.query,
        audit: {},
        steps: [{
          sequence: 1, step_id: "literature-step-1", source: "public_literature", trigger: "missing_evidence_record",
          observation_kind: "neuroanatomy",
          query: { source: "public_literature", query: { specialty: "chiari_malformation", limit: 2 } },
          fallback_to_specialty_scan: false, status: "candidates_found", total_matches: 2, returned_matches: 2, truncated: false,
          references: [{ source: "public_literature", source_id: "pubmed_chiari", record_id: "ACQ-PMID", title: "A bounded Chiari citation", uri: "https://pubmed.ncbi.nlm.nih.gov/ACQ-PMID/" }],
        }],
        candidate_step_count: 1, omitted_step_count: 0, truncated: false, source_query_count: 1, source_candidate_count: 2,
        required_sources: ["public_literature"], ready_for_local_replay: true, human_review_required: true, provider: "none", network: false,
        effect: "read_only", limitations: ["local query worklist only"],
      });
    }
    return originalCallTool(name, args);
  };
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let turns = 0;
  runtime.registerInMemoryProvider("ollama", (request) => {
    turns += 1;
    if (turns === 1) {
      assert.equal(request.tools.some((tool) => tool.name === NEUROSURGERY_GROUNDED_LITERATURE_EVIDENCE_ACQUISITION_TOOL), true);
      return { toolCalls: [{ id: "literature-acquisition-view", name: NEUROSURGERY_GROUNDED_LITERATURE_EVIDENCE_ACQUISITION_TOOL,
        arguments: { max_steps: 64, max_references_per_step: 16 } }] };
    }
    return { structured: {
      answer: "The bounded worker found a PubMed metadata query for reviewer replay.", unknowns: [], claims: [{
        claim_id: "literature-acquisition-plan", kind: "limitation", scope: "citation_metadata",
        text: "The next-evidence plan is a reviewer-owned local query, not a clinical finding.",
        citations: [{ record_kind: "literature_article", record_id: "ACQ-PMID" }],
      }],
    } };
  });
  const result = await agent.groundedPublicLiteratureResearch(
    "Find the next bounded Chiari literature evidence wave.",
    { schema_version: "bioprism-neurosurgery-public-literature/0.1", synthetic_data: false },
    runtime, "ollama", "llama3.1",
    { specialty: "chiari_malformation", approveProviderCall: true, toolLoop: true,
      publicLiteratureQuery: { specialty: "chiari_malformation", limit: 2 } },
  );
  const acquisitionCall = client.calls.find((call) => call.name === NEUROSURGERY_EVIDENCE_ACQUISITION_TOOL);
  assert.deepEqual(acquisitionCall.args.query, { max_steps: 2, max_references_per_step: 16 });
  assert.equal(result.tool_trace[0].view, "evidence_acquisition");
  assert.equal(result.tool_trace[0].plan_digest, "l".repeat(64));
  assert.equal(result.tool_trace[0].returned_steps, 1);
  assert.equal(result.claims[0].citations[0].record_id, "ACQ-PMID");
  assert.equal(result.human_review_required, true);
});

test("grounded public-literature tool loop exposes caller-clocked freshness", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let turns = 0;
  runtime.registerInMemoryProvider("ollama", (request) => {
    turns += 1;
    if (turns === 1) {
      assert.equal(request.tools.some((tool) => tool.name === NEUROSURGERY_GROUNDED_LITERATURE_FRESHNESS_TOOL), true);
      return { toolCalls: [{ id: "literature-freshness", name: NEUROSURGERY_GROUNDED_LITERATURE_FRESHNESS_TOOL, arguments: { max_sources: 4 } }] };
    }
    return { structured: {
      answer: "The caller-clocked literature snapshot is current for review.", unknowns: [], claims: [{
        claim_id: "literature-freshness", kind: "limitation", scope: "citation_metadata",
        text: "Source age remains a caller-clocked metadata state, not a clinical finding.",
        citations: [{ record_kind: "literature_article", record_id: "12345678" }],
      }],
    } };
  });
  const result = await agent.groundedPublicLiteratureResearch(
    "Check the freshness of the glioma literature snapshot.",
    { schema_version: "bioprism-neurosurgery-public-literature/0.1", synthetic_data: false },
    runtime, "ollama", "llama3.1",
    { specialty: "glioma", approveProviderCall: true, toolLoop: true, freshness: { as_of: "2026-08-31T00:00:00Z", max_age_days: 180 }, publicLiteratureQuery: { specialty: "glioma", limit: 1 } },
  );
  const freshnessCall = client.calls.find((call) => call.name === NEUROSURGERY_PUBLIC_LITERATURE_FRESHNESS_TOOL);
  assert.deepEqual(freshnessCall.args.query, { as_of: "2026-08-31T00:00:00Z", max_age_days: 180 });
  assert.equal(result.tool_trace[0].view, "freshness");
  assert.equal(result.tool_trace[0].freshness_digest, "f".repeat(64));
});

test("grounded public-literature tool loop exposes integrity audit", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let turns = 0;
  runtime.registerInMemoryProvider("ollama", (request) => {
    turns += 1;
    if (turns === 1) {
      assert.equal(request.tools.some((tool) => tool.name === NEUROSURGERY_GROUNDED_LITERATURE_INTEGRITY_TOOL), true);
      return { toolCalls: [{ id: "literature-integrity", name: NEUROSURGERY_GROUNDED_LITERATURE_INTEGRITY_TOOL, arguments: { max_issues: 8 } }] };
    }
    return { structured: {
      answer: "The PubMed snapshot has an explicit metadata integrity obligation.", unknowns: [], claims: [{
        claim_id: "literature-integrity", kind: "limitation", scope: "citation_metadata",
        text: "A missing abstract is a reviewer-owned metadata issue and is not negative evidence.",
        citations: [{ record_kind: "literature_article", record_id: "PMID-12345678" }],
      }],
    } };
  });
  const result = await agent.groundedPublicLiteratureResearch(
    "Audit integrity of the glioma literature snapshot.",
    { schema_version: "bioprism-neurosurgery-public-literature/0.1", synthetic_data: false },
    runtime, "ollama", "llama3.1",
    { specialty: "glioma", approveProviderCall: true, toolLoop: true, publicLiteratureQuery: { specialty: "glioma", limit: 1 } },
  );
  const integrityCall = client.calls.find((call) => call.name === NEUROSURGERY_PUBLIC_LITERATURE_INTEGRITY_AUDIT_TOOL);
  assert.deepEqual(integrityCall.args.query, { max_issues: 8, specialties: ["glioma"] });
  assert.equal(result.tool_trace[0].view, "integrity");
  assert.equal(result.tool_trace[0].audit_digest, "i".repeat(64));
  assert.equal(result.tool_trace[0].returned_issues, 1);
  assert.equal(result.claims[0].citations[0].record_id, "PMID-12345678");
});

test("grounded public-literature tool loop exposes the specialist evidence map", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let turns = 0;
  runtime.registerInMemoryProvider("ollama", (request) => {
    turns += 1;
    if (turns === 1) {
      assert.equal(request.tools.some((tool) => tool.name === NEUROSURGERY_GROUNDED_SPECIALTY_EVIDENCE_MAP_TOOL), true);
      return { toolCalls: [{ id: "literature-map", name: NEUROSURGERY_GROUNDED_SPECIALTY_EVIDENCE_MAP_TOOL, arguments: { max_dimensions: 4 } }] };
    }
    return { structured: {
      answer: "The specialist map reports a bounded literature coverage hold.", unknowns: [], claims: [{
        claim_id: "literature-map-hold", kind: "limitation", scope: "citation_metadata",
        text: "The specialist map is reviewer planning metadata, not a clinical finding.",
        citations: [{ record_kind: "literature_article", record_id: "12345678" }],
      }],
    } };
  });
  const result = await agent.groundedPublicLiteratureResearch(
    "Map the specialist glioma literature coverage.",
    { schema_version: "bioprism-neurosurgery-public-literature/0.1", synthetic_data: false },
    runtime, "ollama", "llama3.1",
    { specialty: "glioma", approveProviderCall: true, toolLoop: true, publicLiteratureQuery: { specialty: "glioma", limit: 1 } },
  );
  const mapCall = client.calls.find((call) => call.name === NEUROSURGERY_SPECIALTY_EVIDENCE_MAP_TOOL);
  assert.equal(mapCall.args.request.specialty, "glioma");
  assert.equal(result.tool_trace[0].view, "specialty_evidence_map");
  assert.equal(result.tool_trace[0].map_digest, "m".repeat(64));
  assert.equal(result.tool_trace[0].returned_dimensions, 1);
  assert.equal(result.claims[0].citations[0].record_id, "12345678");
});

test("grounded public-literature evidence acquisition rejects incompatible caller facets", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let turns = 0;
  runtime.registerInMemoryProvider("ollama", (request) => {
    turns += 1;
    if (turns === 1) {
      return { toolCalls: [{ id: "literature-acquisition-facet", name: NEUROSURGERY_GROUNDED_LITERATURE_EVIDENCE_ACQUISITION_TOOL, arguments: {} }] };
    }
    return { structured: {
      answer: "The acquisition request was constrained.", unknowns: [], claims: [{
        claim_id: "context-claim", kind: "source_observation", scope: "citation_metadata",
        text: "The supplied context remains the only citation source.",
        citations: [{ record_kind: "literature_article", record_id: "12345678" }],
      }],
    } };
  });
  const result = await agent.groundedPublicLiteratureResearch(
    "Find a bounded Chiari literature evidence wave.",
    { schema_version: "bioprism-neurosurgery-public-literature/0.1", synthetic_data: false },
    runtime, "ollama", "llama3.1",
    { specialty: "chiari_malformation", approveProviderCall: true, toolLoop: true,
      publicLiteratureQuery: { specialty: "chiari_malformation", publication_type: "Review", limit: 1 } },
  );
  assert.equal(result.tool_trace[0].status, "error");
  assert.match(result.tool_trace[0].error, /publication_type/);
  assert.equal(client.calls.some((call) => call.name === NEUROSURGERY_EVIDENCE_ACQUISITION_TOOL), false);
});

test("grounded public-literature review queue rejects incompatible caller facets", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let turns = 0;
  runtime.registerInMemoryProvider("ollama", () => {
    turns += 1;
    if (turns === 1) return { toolCalls: [{ id: "queue-facet", name: NEUROSURGERY_GROUNDED_LITERATURE_REVIEW_QUEUE_TOOL, arguments: {} }] };
    return { structured: {
      answer: "The queue request was constrained.", unknowns: [], claims: [{
        claim_id: "context-claim", kind: "source_observation", scope: "citation_metadata",
        text: "The supplied context remains the only citation source.",
        citations: [{ record_kind: "literature_article", record_id: "12345678" }],
      }],
    } };
  });
  const result = await agent.groundedPublicLiteratureResearch(
    "Review Chiari corpus completeness.",
    { schema_version: "bioprism-neurosurgery-public-literature/0.1", synthetic_data: false },
    runtime, "ollama", "llama3.1",
    { specialty: "chiari_malformation", approveProviderCall: true, toolLoop: true,
      publicLiteratureQuery: { specialty: "chiari_malformation", publication_type: "Review", limit: 1 } },
  );
  assert.equal(result.tool_trace[0].status, "error");
  assert.match(result.tool_trace[0].error, /publication_type/);
  assert.equal(client.calls.some((call) => call.name === NEUROSURGERY_PUBLIC_LITERATURE_REVIEW_QUEUE_TOOL), false);
});

test("grounded bridges reject credentialless remote HTTP providers", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  runtime.registerProvider(openaiCompatibleProvider("remote-no-key", "https://gateway.example.invalid/v1", {
    requiresCredential: false,
  }));
  const realBundle = { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false, sources: [] };
  const literatureBundle = { schema_version: "bioprism-neurosurgery-public-literature/0.1", synthetic_data: false, sources: [], records: [] };
  await assert.rejects(
    () => agent.groundedRealDataResearch("summarize", realBundle, runtime, "remote-no-key", "model", { approveProviderCall: true }),
    ArgumentError,
  );
  await assert.rejects(
    () => agent.groundedPublicLiteratureResearch("summarize", literatureBundle, runtime, "remote-no-key", "model", { approveProviderCall: true }),
    ArgumentError,
  );
  assert.equal(client.calls.length, 0);
});

test("grounded research loops expand unknowns and terminate at a bounded review ledger", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let calls = 0;
  runtime.registerInMemoryProvider("ollama", () => {
    calls += 1;
    return {
      structured: {
        answer: `Pass ${calls} is a source-bound observation.`,
        unknowns: calls === 1 ? ["verify missing publication linkage"] : [],
        claims: [{
          claim_id: `claim-${calls}`,
          kind: "population_summary",
          scope: "population_aggregate",
          text: "The supplied public snapshot remains a population metadata source.",
          citations: [{ record_kind: "genomic_project", record_id: "TCGA-GBM" }],
        }],
      },
    };
  });
  const bundle = { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false, sources: [] };
  const result = await agent.groundedRealDataResearchLoop(
    "Summarize the available glioma population metadata.", bundle, runtime, "ollama", "llama3.1",
    { approveProviderCall: true, maxPasses: 3, maxFollowUpsPerPass: 2, includeAbstracts: false },
  );
  assert.equal(result.schema_version, "bioprism-neurosurgery-grounded-research-loop/0.1");
  assert.equal(result.completed_pass_count, 2);
  assert.equal(result.termination, "no_new_queries");
  assert.equal(result.pending_queries.length, 0);
  assert.equal(result.claim_count, 2);
  assert.equal(result.status, "grounded_for_human_review");
  assert.equal(result.passes[0].follow_up_queries.length, 1);
  assert.equal(result.passes[1].follow_up_queries.length, 0);
  assert.equal(result.passes[0].claim_digest.length, 64);
  assert.equal(result.passes[1].claim_digest.length, 64);
  assert.equal(result.passes[0].audit_digest.length, 64);
  assert.equal(result.passes[1].audit_digest.length, 64);
  assert.equal(calls, 2);
  assert.equal(client.calls.filter((call) => call.name === NEUROSURGERY_REAL_DATA_REASONING_CONTEXT_TOOL).length, 2);
  assert.equal(client.calls.filter((call) => call.name === NEUROSURGERY_REAL_DATA_DRAFT_AUDIT_TOOL).length, 2);
});

test("grounded research loops resume a tamper-evident pending ledger", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let calls = 0;
  runtime.registerInMemoryProvider("ollama", () => {
    calls += 1;
    return {
      structured: {
        answer: `Resumable pass ${calls}.`,
        unknowns: calls === 1 ? ["check the source refresh timestamp"] : [],
        claims: [{
          claim_id: `resume-claim-${calls}`,
          kind: "population_summary",
          scope: "population_aggregate",
          text: "The supplied snapshot is a population metadata source.",
          citations: [{ record_kind: "genomic_project", record_id: "TCGA-GBM" }],
        }],
      },
    };
  });
  const bundle = { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false, sources: [] };
  const checkpoint = await agent.groundedRealDataResearchLoop(
    "Summarize the available glioma population metadata.", bundle, runtime, "ollama", "llama3.1",
    { approveProviderCall: true, maxPasses: 1, maxFollowUpsPerPass: 1, includeAbstracts: false },
  );
  assert.equal(checkpoint.termination, "max_passes_reached");
  assert.equal(checkpoint.pending_queries.length, 1);
  assert.equal(checkpoint.status, "incomplete_budget");
  assert.deepEqual(checkpoint.research_policy, {
    max_follow_ups_per_pass: 1,
    max_output_tokens: 2048,
    max_hits: 32,
    max_chars: 24000,
    include_abstracts: false,
    freshness: null,
    tool_loop: false,
    max_tool_turns: 4,
    max_tool_calls: 8,
  });
  await assert.rejects(
    () => agent.groundedRealDataResearchLoop(
      "Summarize the available glioma population metadata.", bundle, runtime, "ollama", "llama3.1",
      { approveProviderCall: true, maxPasses: 2, maxFollowUpsPerPass: 1, maxChars: 12000, includeAbstracts: false, resumeFrom: checkpoint },
    ),
    ArgumentError,
  );
  assert.equal(calls, 1);
  const resumed = await agent.groundedRealDataResearchLoop(
    "Summarize the available glioma population metadata.", bundle, runtime, "ollama", "llama3.1",
    { approveProviderCall: true, maxPasses: 2, maxFollowUpsPerPass: 1, includeAbstracts: false, resumeFrom: checkpoint },
  );
  assert.equal(resumed.completed_pass_count, 2);
  assert.equal(resumed.termination, "no_new_queries");
  assert.equal(resumed.pending_queries.length, 0);
  assert.equal(resumed.status, "grounded_for_human_review");
  assert.equal(calls, 2);
  await assert.rejects(
    () => agent.groundedRealDataResearchLoop(
      "Summarize the available glioma population metadata.", bundle, runtime, "ollama", "llama3.1",
      { approveProviderCall: true, maxPasses: 2, resumeFrom: { ...checkpoint, loop_digest: "tampered" } },
    ),
    ArgumentError,
  );
  const tamperedPass = {
    ...checkpoint.passes[0],
    claims: [{ ...checkpoint.passes[0].claims[0], text: "tampered claim payload" }],
  };
  await assert.rejects(
    () => agent.groundedRealDataResearchLoop(
      "Summarize the available glioma population metadata.", bundle, runtime, "ollama", "llama3.1",
      { approveProviderCall: true, maxPasses: 2, resumeFrom: { ...checkpoint, passes: [tamperedPass] } },
    ),
    ArgumentError,
  );
  const tamperedAudit = structuredClone(checkpoint);
  tamperedAudit.passes[0].audit.grounded_claim_count += 1;
  await assert.rejects(
    () => agent.groundedRealDataResearchLoop(
      "Summarize the available glioma population metadata.", bundle, runtime, "ollama", "llama3.1",
      { approveProviderCall: true, maxPasses: 2, maxFollowUpsPerPass: 1, includeAbstracts: false, resumeFrom: tamperedAudit },
    ),
    ArgumentError,
  );
  await assert.rejects(
    () => agent.groundedRealDataResearchLoop(
      "Summarize the available glioma population metadata.", bundle, runtime, "ollama", "llama3.1",
      { approveProviderCall: true, maxPasses: 2, maxFollowUpsPerPass: 1, includeAbstracts: false, resumeFrom: { ...checkpoint, grounded_claim_count: 999 } },
    ),
    ArgumentError,
  );
});

test("grounded real-data loops bind query facets and reject resume drift", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  runtime.registerInMemoryProvider("ollama", () => ({
    structured: {
      answer: "The filtered registry metadata is a source-bound observation.",
      unknowns: [],
      claims: [{
        claim_id: "filtered-trials",
        kind: "population_summary",
        scope: "public_record_metadata",
        text: "The selected trial slice is limited to interventional studies.",
        citations: [{ record_kind: "clinical_trial", record_id: "NCT00000001" }],
      }],
    },
  }));
  const bundle = { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false, sources: [] };
  const realDataQuery = {
    record_kind: "clinical_trial",
    trial_study_type: "Interventional",
    trial_updated_from: "2024-01-01",
    trial_updated_to: "2024-12-31",
    limit: 2,
  };
  const checkpoint = await agent.groundedRealDataResearchLoop(
    "Summarize interventional glioma trials.", bundle, runtime, "ollama", "llama3.1",
    { approveProviderCall: true, maxPasses: 1, includeAbstracts: false, realDataQuery },
  );
  assert.deepEqual(checkpoint.real_data_query, {
    ...realDataQuery,
    text: "Summarize interventional glioma trials.",
  });
  const contextCall = client.calls.find((call) => call.name === NEUROSURGERY_REAL_DATA_REASONING_CONTEXT_TOOL);
  assert.deepEqual(contextCall.args.query.packet.query, checkpoint.real_data_query);
  await assert.rejects(
    () => agent.groundedRealDataResearchLoop(
      "Summarize interventional glioma trials.", bundle, runtime, "ollama", "llama3.1",
      {
        approveProviderCall: true,
        maxPasses: 2,
        realDataQuery: { ...realDataQuery, trial_study_type: "Observational" },
        resumeFrom: checkpoint,
      },
    ),
    ArgumentError,
  );
});

test("grounded real-data loops execute follow-up text with explicit facets", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let calls = 0;
  runtime.registerInMemoryProvider("ollama", () => {
    calls += 1;
    return {
      structured: {
        answer: `Pass ${calls}.`,
        unknowns: calls === 1 ? ["confirm linked publication metadata"] : [],
        claims: [{
          claim_id: `facet-follow-up-${calls}`,
          kind: "source_observation",
          scope: "public_record_metadata",
          text: "The bounded source context remains metadata-only.",
          citations: [{ record_kind: "portal_molecular_profile", record_id: "profile-1" }],
        }],
      },
    };
  });
  const bundle = { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false, sources: [] };
  const realDataQuery = {
    text: "IDH molecular profile",
    record_kind: "portal_molecular_profile",
    molecular_alteration_type: "MUTATION_EXTENDED",
    limit: 1,
  };
  const result = await agent.groundedRealDataResearchLoop(
    "Summarize glioma molecular metadata.", bundle, runtime, "ollama", "llama3.1",
    { approveProviderCall: true, maxPasses: 2, maxFollowUpsPerPass: 1, includeAbstracts: false, realDataQuery },
  );
  const contextQueries = client.calls
    .filter((call) => call.name === NEUROSURGERY_REAL_DATA_REASONING_CONTEXT_TOOL)
    .map((call) => call.args.query.packet.query);
  assert.equal(contextQueries.length, 2);
  assert.equal(contextQueries[0].text, realDataQuery.text);
  assert.equal(contextQueries[1].text, "evidence metadata gap: confirm linked publication metadata");
  assert.equal(contextQueries[0].record_kind, contextQueries[1].record_kind);
  assert.equal(result.completed_pass_count, 2);
  assert.equal(calls, 2);
});

test("grounded literature loops preserve specialty and PMID audit boundaries", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let calls = 0;
  runtime.registerInMemoryProvider("ollama", () => {
    calls += 1;
    return {
      structured: {
        answer: "The bounded PubMed lane remains a citation handoff.",
        unknowns: calls === 1 ? ["verify abstract availability"] : [],
        claims: [{
          claim_id: `literature-claim-${calls}`,
          kind: "source_observation",
          scope: "citation_metadata",
          text: "The selected specialty lane contains source-linked metadata.",
          citations: [{ record_kind: "literature_article", record_id: "12345678" }],
        }],
      },
    };
  });
  const bundle = { schema_version: "bioprism-neurosurgery-public-literature/0.1", synthetic_data: false, sources: [], records: [] };
  const result = await agent.groundedPublicLiteratureResearchLoop(
    "Summarize source-linked Chiari literature.", bundle, runtime, "ollama", "llama3.1",
    { specialty: "chiari_malformation", approveProviderCall: true, maxPasses: 2, maxFollowUpsPerPass: 1, includeAbstracts: false },
  );
  assert.equal(result.schema_version, "bioprism-neurosurgery-grounded-literature-research-loop/0.1");
  assert.equal(result.specialty, "chiari_malformation");
  assert.equal(result.completed_pass_count, 2);
  assert.equal(result.termination, "no_new_queries");
  assert.equal(result.status, "grounded_for_human_review");
  assert.equal(calls, 2);
  assert.equal(client.calls.filter((call) => call.name === NEUROSURGERY_PUBLIC_LITERATURE_REASONING_CONTEXT_TOOL).length, 2);
  assert.equal(client.calls.filter((call) => call.name === NEUROSURGERY_PUBLIC_LITERATURE_DRAFT_AUDIT_TOOL).length, 2);
});

test("grounded literature loops preserve structured PubMed facets and resume identity", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let calls = 0;
  runtime.registerInMemoryProvider("ollama", () => {
    calls += 1;
    return {
      structured: {
        answer: `Pass ${calls}.`,
        unknowns: calls === 1 ? ["check date-bounded review coverage"] : [],
        claims: [{
          claim_id: `facet-literature-${calls}`,
          kind: "source_observation",
          scope: "citation_metadata",
          text: "The bounded PubMed context remains source-linked metadata.",
          citations: [{ record_kind: "literature_article", record_id: "12345678" }],
        }],
      },
    };
  });
  const bundle = { schema_version: "bioprism-neurosurgery-public-literature/0.1", synthetic_data: false, sources: [], records: [] };
  const publicLiteratureQuery = {
    specialty: "glioma",
    text: "IDH glioma reviews",
    publication_type: "Review",
    mesh_term: "Glioma",
    from_date: "2020-01-01",
    to_date: "2024-12-31",
    limit: 7,
  };
  const checkpoint = await agent.groundedPublicLiteratureResearchLoop(
    "Summarize glioma evidence.", bundle, runtime, "ollama", "llama3.1",
    { approveProviderCall: true, maxPasses: 1, maxFollowUpsPerPass: 1, includeAbstracts: false, publicLiteratureQuery },
  );
  assert.equal(checkpoint.termination, "max_passes_reached");
  assert.equal(checkpoint.status, "incomplete_budget");
  assert.equal(checkpoint.pending_queries.length, 1);
  const resumed = await agent.groundedPublicLiteratureResearchLoop(
    "Summarize glioma evidence.", bundle, runtime, "ollama", "llama3.1",
    {
      approveProviderCall: true,
      maxPasses: 2,
      maxFollowUpsPerPass: 1,
      includeAbstracts: false,
      publicLiteratureQuery,
      resumeFrom: checkpoint,
    },
  );
  assert.equal(resumed.status, "grounded_for_human_review");
  assert.equal(resumed.pending_queries.length, 0);
  const contextQueries = client.calls
    .filter((call) => call.name === NEUROSURGERY_PUBLIC_LITERATURE_REASONING_CONTEXT_TOOL)
    .map((call) => call.args.query.packet.query);
  assert.deepEqual(contextQueries[0], publicLiteratureQuery);
  assert.equal(contextQueries[1].publication_type, "Review");
  assert.equal(contextQueries[1].mesh_term, "Glioma");
  assert.equal(contextQueries[1].from_date, "2020-01-01");
  assert.equal(contextQueries[1].to_date, "2024-12-31");
  assert.equal(contextQueries[1].text, "evidence metadata gap: check date-bounded review coverage");
  assert.deepEqual(checkpoint.public_literature_query, publicLiteratureQuery);
  assert.equal(checkpoint.loop_digest.length, 64);
  await assert.rejects(
    () => agent.groundedPublicLiteratureResearchLoop(
      "Summarize glioma evidence.", bundle, runtime, "ollama", "llama3.1",
      {
        approveProviderCall: true,
        maxPasses: 2,
        publicLiteratureQuery: { ...publicLiteratureQuery, mesh_term: "Glioblastoma" },
        resumeFrom: checkpoint,
      },
    ),
    ArgumentError,
  );
});

test("grounded research portfolio coordinates real and PubMed planes without blending them", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let calls = 0;
  runtime.registerInMemoryProvider("ollama", () => {
    calls += 1;
    return {
      structured: {
        answer: "Source-separated portfolio handoff.",
        unknowns: [],
        claims: [{
          claim_id: `portfolio-claim-${calls}`,
          kind: "source_observation",
          scope: "citation_metadata",
          text: "The selected source plane contains a bounded public record.",
          citations: [{
            record_kind: calls === 1 ? "genomic_project" : "literature_article",
            record_id: calls === 1 ? "TCGA-GBM" : "12345678",
          }],
        }],
      },
    };
  });
  const result = await agent.groundedResearchPortfolio(
    "Summarize source-linked glioma evidence.", runtime, "ollama", "llama3.1",
    {
      realGliomaData: { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false, sources: [] },
      publicLiterature: { schema_version: "bioprism-neurosurgery-public-literature/0.1", synthetic_data: false, sources: [], records: [] },
      specialty: "glioma",
      approveProviderCall: true,
      maxPasses: 1,
      maxFollowUpsPerPass: 0,
      includeAbstracts: false,
      realDataQuery: {
        record_kind: "genomic_project",
        genomic_data_type: "Annotated Somatic Mutation",
        limit: 1,
      },
    },
  );
  assert.equal(result.schema_version, NEUROSURGERY_GROUNDED_RESEARCH_PORTFOLIO_SCHEMA);
  assert.deepEqual(result.source_planes, ["real_glioma_population", "public_literature"]);
  assert.equal(result.real_data_loop?.bundle_digest, "b".repeat(64));
  assert.deepEqual(result.real_data_query, {
    record_kind: "genomic_project",
    genomic_data_type: "Annotated Somatic Mutation",
    limit: 1,
    text: "Summarize source-linked glioma evidence.",
  });
  assert.equal(result.public_literature_loop?.bundle_digest, "f".repeat(64));
  assert.equal(result.literature_link_audit?.audit_digest, "l".repeat(64));
  assert.deepEqual(result.literature_link_audit?.query, {
    public_specialty: "glioma",
    max_links: 32,
    max_unmatched_ids: 32,
  });
  assert.equal(result.completed_pass_count, 2);
  assert.equal(result.claim_count, 2);
  assert.equal(result.status, "grounded_for_human_review");
  assert.equal(result.human_review_required, true);
  assert.equal(calls, 2);
});

test("grounded research portfolio refuses a synthetic link audit", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  runtime.registerInMemoryProvider("ollama", () => ({
    structured: {
      answer: "bounded source handoff",
      unknowns: [],
      claims: [{
        claim_id: "link-boundary",
        kind: "source_observation",
        scope: "citation_metadata",
        text: "A bounded source record is available for review.",
        citations: [{ record_kind: "literature_article", record_id: "12345678" }],
      }],
    },
  }));
  agent.literatureLinkAudit = async () => ({
    synthetic_data: true,
    network: false,
    provenance_bound: true,
    human_review_required: true,
    provider: "none",
    effect: "read_only",
  });
  const bundle = {
    schema_version: "bioprism-neurosurgery-real/0.1",
    synthetic_data: false,
    sources: [],
  };
  const literature = {
    schema_version: "bioprism-neurosurgery-public-literature/0.1",
    synthetic_data: false,
    sources: [],
    records: [],
  };
  await assert.rejects(
    () => agent.groundedResearchPortfolio(
      "Summarize source-linked glioma evidence.", runtime, "ollama", "llama3.1",
      {
        realGliomaData: bundle,
        publicLiterature: literature,
        specialty: "glioma",
        approveProviderCall: true,
        maxPasses: 1,
        maxFollowUpsPerPass: 0,
        includeAbstracts: false,
      },
    ),
    ProtocolError,
  );
});

test("grounded research portfolio carries a deidentified case-asset projection", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  runtime.registerInMemoryProvider("ollama", () => ({
    structured: {
      answer: "bounded real-data handoff",
      unknowns: [],
      claims: [{
        claim_id: "case-asset-handoff",
        kind: "source_observation",
        scope: "population_aggregate",
        text: "The source snapshot exposes a bounded aggregate project record.",
        citations: [{ record_kind: "genomic_project", record_id: "TCGA-GBM" }],
      }],
    },
  }));
  const result = await agent.groundedResearchPortfolio(
    "Summarize the real glioma metadata and attached case inventory.", runtime, "ollama", "llama3.1",
    {
      realGliomaData: { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false, sources: [] },
      specialty: "glioma",
      caseAssetManifest: {
        schema_version: "bioprism-neurosurgery-case-asset-manifest/0.1",
        specialty: "glioma",
        synthetic_data: false,
        assets: [],
      },
      caseAssetManifestQuery: { requested_kinds: ["imaging_series"], max_review_items: 16 },
      approveProviderCall: true,
      maxPasses: 1,
      maxFollowUpsPerPass: 0,
      includeAbstracts: false,
    },
  );
  assert.equal(result.case_asset_manifest?.report_digest, "d".repeat(64));
  assert.deepEqual(result.case_asset_manifest_query, {
    requested_kinds: ["imaging_series"],
    max_review_items: 16,
  });
  assert.deepEqual(
    client.calls.filter((call) => call.name === NEUROSURGERY_CASE_ASSET_MANIFEST_TOOL).map((call) => call.name),
    [NEUROSURGERY_CASE_ASSET_MANIFEST_TOOL],
  );
});

test("grounded research intake gates a missing real glioma snapshot before any model call", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let providerCalls = 0;
  runtime.registerInMemoryProvider("ollama", () => {
    providerCalls += 1;
    return { structured: { answer: "must not run", unknowns: [], claims: [] } };
  });
  const result = await agent.groundedResearchIntake(
    "What does the glioma molecular evidence contain?", runtime, "ollama", "llama3.1",
    { approveProviderCall: true },
  );
  assert.equal(result.schema_version, NEUROSURGERY_GROUNDED_RESEARCH_INTAKE_SCHEMA);
  assert.equal(result.status, "needs_evidence");
  assert.equal(result.routed_specialty, "glioma");
  assert.deepEqual(result.required_evidence, ["real_glioma_snapshot"]);
  assert.equal(result.portfolio, null);
  assert.equal(providerCalls, 0);
  assert.deepEqual(client.calls.map((call) => call.name), [NEUROSURGERY_INTAKE_PLAN_TOOL]);
});

test("grounded research intake carries a deidentified case-asset projection", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  runtime.registerInMemoryProvider("ollama", () => ({
    structured: {
      answer: "bounded real-data handoff",
      unknowns: [],
      claims: [{
        claim_id: "intake-case-asset-handoff",
        kind: "source_observation",
        scope: "population_aggregate",
        text: "The source snapshot exposes a bounded aggregate project record.",
        citations: [{ record_kind: "genomic_project", record_id: "TCGA-GBM" }],
      }],
    },
  }));
  const result = await agent.groundedResearchIntake(
    "What does the glioma molecular evidence contain?", runtime, "ollama", "llama3.1",
    {
      realGliomaData: { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false, sources: [] },
      caseAssetManifest: {
        schema_version: "bioprism-neurosurgery-case-asset-manifest/0.1",
        specialty: "glioma",
        synthetic_data: false,
        assets: [],
      },
      caseAssetManifestQuery: { requested_kinds: ["imaging_series"], max_review_items: 16 },
      approveProviderCall: true,
      maxPasses: 1,
      maxFollowUpsPerPass: 0,
      includeAbstracts: false,
    },
  );
  assert.equal(result.status, "grounded_for_human_review");
  const realContextCall = client.calls.find((call) => call.name === NEUROSURGERY_REAL_DATA_REASONING_CONTEXT_TOOL);
  assert.equal(realContextCall?.args.query.packet.query.text, "glioma");
  assert.equal(realContextCall?.args.query.packet.query.text.includes("What does the glioma molecular evidence contain?"), false);
  assert.equal(result.portfolio?.case_asset_manifest?.report_digest, "d".repeat(64));
  assert.deepEqual(result.portfolio?.case_asset_manifest_query, {
    requested_kinds: ["imaging_series"],
    max_review_items: 16,
  });
  assert.equal(
    client.calls.filter((call) => call.name === NEUROSURGERY_CASE_ASSET_MANIFEST_TOOL).length,
    1,
  );
});

test("grounded research intake routes non-glioma questions to the public plane only", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const runtime = new LLMRuntime();
  let providerCalls = 0;
  runtime.registerInMemoryProvider("ollama", () => {
    providerCalls += 1;
    return {
      structured: {
        answer: "Source-separated congenital literature handoff.",
        unknowns: ["verify the unreported follow-up horizon"],
        claims: [{
          claim_id: "chiari-intake-claim",
          kind: "source_observation",
          scope: "citation_metadata",
          text: "The selected public literature lane contains a bounded citation record.",
          citations: [{ record_kind: "literature_article", record_id: "12345678" }],
        }],
      },
    };
  });
  agent.intakePlan = async () => ({
    schema_version: "bioprism-neurosurgery-intake-plan/0.1",
    plan_digest: "i".repeat(64),
    question_digest: "q".repeat(64),
    candidates: [{ specialty: "chiari_malformation", score_bps: 1000, matched_terms: ["chiari"] }],
    selected_specialty: "chiari_malformation",
    confidence_bps: 1000,
    abstained: false,
    reason: "selected",
    route: ["safety_gate", "public_literature", "human_review_hold"],
    evidence_sources: ["pubmed_snapshot"],
    reviewer_roles: ["neurosurgery"],
    next_actions: [],
    human_review_required: true,
    provider: "none",
    network: false,
    effect: "read_only",
    limitations: [],
  });
  const result = await agent.groundedResearchIntake(
    "What source-linked Chiari literature is available?", runtime, "ollama", "llama3.1",
    {
      publicLiterature: { schema_version: "bioprism-neurosurgery-public-literature/0.1", synthetic_data: false, sources: [], records: [] },
      approveProviderCall: true,
      maxPasses: 1,
      maxFollowUpsPerPass: 1,
      includeAbstracts: false,
    },
  );
  assert.equal(result.status, "incomplete_budget");
  assert.equal(result.routed_specialty, "chiari_malformation");
  assert.deepEqual(result.source_planes, ["public_literature"]);
  assert.ok(result.portfolio);
  assert.equal(result.portfolio.status, "incomplete_budget");
  assert.deepEqual(result.portfolio.public_literature_loop?.pending_queries, [
    "evidence metadata gap: verify the unreported follow-up horizon",
  ]);
  assert.equal(providerCalls, 1);
  assert.equal(client.calls.some((call) => call.name === NEUROSURGERY_REAL_DATA_REASONING_CONTEXT_TOOL), false);
  assert.equal(client.calls.some((call) => call.name === NEUROSURGERY_PUBLIC_LITERATURE_REASONING_CONTEXT_TOOL), true);
});

test("facade queries and plans against the source-linked public literature bundle", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const bundle = { schema_version: "bioprism-neurosurgery-public-literature/0.1", synthetic_data: false, sources: [], records: [] };
  const result = await agent.queryPublicLiterature(bundle, {
    specialty: "glioma",
    text: "molecular",
    publication_type: "review",
    mesh_term: "glioma",
    from_date: "2020-01-01",
    to_date: "2025-12-31",
    limit: 4,
  });
  assert.equal(result.returned_matches, 1);
  const queryCall = client.calls.at(-1);
  assert.equal(queryCall.name, NEUROSURGERY_PUBLIC_LITERATURE_QUERY_TOOL);
  assert.equal(queryCall.args.query.specialty, "glioma");
  assert.equal(queryCall.args.query.publication_type, "review");
  assert.equal(queryCall.args.query.mesh_term, "glioma");
  assert.equal(queryCall.args.query.from_date, "2020-01-01");
  assert.equal(queryCall.args.query.to_date, "2025-12-31");
  assert.equal(queryCall.args.query.limit, 4);
  const report = await agent.planWithPublicLiterature({ use: "research_synthesis", specialty: "glioma" }, bundle);
  assert.equal(report.specialty, "glioma");
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_TOOL);
  const handoff = await agent.planResearch(
    { use: "research_synthesis", specialty: "glioma" },
    {},
    undefined,
    bundle,
    4,
    2,
  );
  assert.equal(handoff.schema_version, "bioprism-neurosurgery-research-plan/0.1");
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_RESEARCH_PLAN_TOOL);
  assert.equal(client.calls.at(-1).args.public_literature.schema_version, bundle.schema_version);
  await assert.rejects(() => agent.queryPublicLiterature(bundle, { specialty: "not-a-specialty" }), ArgumentError);
  await assert.rejects(() => agent.queryPublicLiterature(bundle, { limit: 129 }), ArgumentError);
  await assert.rejects(() => agent.queryPublicLiterature(bundle, { from_date: "2025-01-01", to_date: "2024-01-01" }), ArgumentError);
});

test("runSession advances a bounded checkpoint chain and finishes only at human review", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const result = await agent.runSession({ use: "research_synthesis", specialty: "glioma" }, {}, undefined, 4);
  assert.equal(result.status, "needs_evidence");
  const advanceCalls = client.calls.filter((call) => call.name === NEUROSURGERY_SESSION_TOOL && call.args.operation === "advance");
  assert.equal(advanceCalls.length, 2);
  assert.equal(client.calls.at(-1).args.operation, "finish");
  const oneCall = await agent.runSessionToReview({ use: "research_synthesis", specialty: "glioma" }, {}, undefined, 2);
  assert.equal(oneCall.steps_executed, 2);
  assert.equal(oneCall.session.status, NEUROSURGERY_SESSION_TERMINAL_STATUS);
  assert.equal((await agent.iterateSession({ use: "research_synthesis", specialty: "glioma" }, {}, undefined, 4).next()).value.status, "planned");
});

test("runResearchMission requires real glioma evidence and composes the guarded stages", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  await assert.rejects(
    () => agent.runResearchMission({ specialty: "glioma", request_use: "research_synthesis" }),
    ArgumentError,
  );
  const mission = await agent.runResearchMission(
    { specialty: "glioma", request_use: "research_synthesis" },
    {},
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    { text: "GBM", limit: 4 },
    2,
    { as_of: "2027-08-31T00:00:00Z", max_age_days: 30 },
  );
  assert.equal(mission.schema, "bioprism-neurosurgical-research-mission/0.1");
  assert.equal(mission.provider, "none");
  assert.equal(mission.network, false);
  assert.equal(mission.human_review_required, true);
  assert.equal(mission.real_data_query.returned_count, 1);
  assert.equal(mission.real_data_review_queue.provider, "none");
  assert.equal(mission.real_data_evidence_packet.network, false);
  assert.equal(client.calls.at(-1).args.freshness.max_age_days, 30);
  assert.equal(mission.real_data_evidence_graph.total_node_count, 88);
  assert.equal(mission.real_data_evidence_graph.provider, "none");
  assert.equal(mission.real_data_reasoning_context.synthetic_data, false);
  assert.equal(mission.real_data_reasoning_context.context_text.includes("AURORA REAL-GLIOMA"), true);
  assert.equal(mission.research_plan.schema_version, "bioprism-neurosurgery-research-plan/0.1");
  assert.equal(mission.research_plan.provider, "none");
  assert.equal(mission.run.steps_executed, 2);
  const dicomImport = {
    schema_version: "bioprism-neurosurgery-case-dicom-import/0.1",
    specialty: "glioma",
    deidentified: true,
    synthetic_data: false,
    source_id: "dicom-export",
    datasets: [],
  };
  await agent.runResearchMission(
    { specialty: "glioma", request_use: "research_synthesis" },
    {},
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    undefined,
    2,
    undefined,
    undefined,
    undefined,
    undefined,
    undefined,
    undefined,
    undefined,
    dicomImport,
  );
  assert.deepEqual(client.calls.at(-1).args.case_dicom_import, dicomImport);
  const fhirImport = {
    schema_version: "bioprism-neurosurgery-case-fhir-import/0.1",
    specialty: "glioma",
    deidentified: true,
    synthetic_data: false,
    source_id: "fhir-export",
    bundle: { resourceType: "Bundle", entry: [] },
  };
  await agent.runResearchMission(
    { specialty: "glioma", request_use: "research_synthesis" },
    {},
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    undefined,
    2,
    undefined,
    undefined,
    undefined,
    undefined,
    undefined,
    undefined,
    undefined,
    undefined,
    fhirImport,
  );
  assert.deepEqual(client.calls.at(-1).args.case_fhir_import, fhirImport);
  await agent.runResearchMission(
    { specialty: "glioma", request_use: "research_synthesis" },
    {},
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    undefined,
    2,
    undefined,
    undefined,
    undefined,
    undefined,
    undefined,
    undefined,
    undefined,
    dicomImport,
    fhirImport,
  );
  assert.deepEqual(client.calls.at(-1).args.case_dicom_import, dicomImport);
  assert.deepEqual(client.calls.at(-1).args.case_fhir_import, fhirImport);
  await assert.rejects(
    () => agent.runResearchMission(
      { specialty: "glioma", request_use: "research_synthesis" },
      {},
      { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
      undefined,
      2,
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      { requested_kinds: ["imaging_series"] },
    ),
    ArgumentError,
  );
  const attachedManifest = {
    schema_version: "bioprism-neurosurgery-case-asset-manifest/0.1",
    specialty: "glioma",
    synthetic_data: false,
    direct_identifier_fields: [],
    assets: [],
  };
  const attached = await agent.runResearchMission(
    { specialty: "glioma", request_use: "research_synthesis" },
    {},
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    undefined,
    2,
    undefined,
    undefined,
    undefined,
    undefined,
    attachedManifest,
    { requested_kinds: ["imaging_series"], max_review_items: 16 },
    {
      schema_version: "bioprism-neurosurgery-case-asset-review-disposition/0.1",
      report_digest: "d".repeat(64),
      disposition_digest: "x".repeat(64),
      candidate_item_count: 0,
      returned_item_count: 0,
      omitted_item_count: 0,
      submitted_decision_count: 0,
      accepted_decision_count: 0,
      resolved_decision_count: 0,
      unresolved_decision_count: 0,
      undecided_returned_item_count: 0,
      pending_item_count: 0,
      decisions: [],
      unresolved_sequences: [],
      undecided_sequences: [],
      provenance_bound: true,
      synthetic_data: false,
      human_review_required: true,
      provider: "none",
      network: false,
      effect: "read_only",
      limitations: [],
    },
  );
  assert.equal(attached.provider, "none");
  assert.deepEqual(client.calls.at(-1).args.case_asset_manifest, attachedManifest);
  assert.deepEqual(client.calls.at(-1).args.case_asset_manifest_query.requested_kinds, ["imaging_series"]);
  assert.equal(client.calls.at(-1).args.case_asset_review_disposition.report_digest, "d".repeat(64));
  const dualMission = await agent.runResearchMission(
    { specialty: "glioma", request_use: "research_synthesis" },
    {},
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    { text: "GBM", limit: 1 },
    2,
    { as_of: "2027-08-31T00:00:00Z", max_age_days: 30 },
    undefined,
    { schema_version: "bioprism-neurosurgery-public-literature/0.1", synthetic_data: false },
    { specialty: "glioma", text: "glioma", limit: 1 },
  );
  assert.equal(dualMission.literature_link_audit.schema_version, "bioprism-neurosurgery-literature-link-audit/0.1");
  assert.equal(client.calls.at(-1).args.public_literature_query.limit, 1);
});

test("validateMission replays a persisted mission through the existing no-key tool", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const result = await agent.validateMission(
    { use: "research_synthesis", specialty: "glioma" },
    { schema: "bioprism-neurosurgical-research-mission/0.1", mission_id: "neurosurgical-mission-test" },
    {},
    { schema_version: "bioprism-neurosurgery-real/0.1", synthetic_data: false },
    undefined,
    undefined,
    { schema_version: "bioprism-neurosurgery-case-fhir-import/0.1" },
  );
  assert.equal(result.valid, true);
  assert.equal(result.provider, "none");
  assert.equal(result.network, false);
  assert.equal(client.calls.at(-1).name, NEUROSURGERY_MISSION_TOOL);
  assert.equal(client.calls.at(-1).args.operation, "validate");
  assert.equal("mission" in client.calls.at(-1).args, true);
  assert.equal(
    client.calls.at(-1).args.case_fhir_import.schema_version,
    "bioprism-neurosurgery-case-fhir-import/0.1",
  );
});

test("public-literature sessions and missions use the same bounded review lifecycle", async () => {
  const client = fakeClient();
  const agent = new LocalNeurosurgicalAgent(client);
  const request = { specialty: "encephalocele", request_use: "research_synthesis" };
  const bundle = { schema_version: "bioprism-neurosurgery-public-literature/0.1", synthetic_data: false };
  await assert.rejects(() => agent.runResearchMission(request, {}, undefined, undefined, 2), ArgumentError);
  const started = await agent.startPublicLiteratureSession(request, bundle);
  assert.equal(started.status, "planned");
  let advanced = await agent.advancePublicLiteratureSession(request, started, bundle);
  while (advanced.status !== NEUROSURGERY_SESSION_TERMINAL_STATUS) {
    advanced = await agent.advancePublicLiteratureSession(request, advanced, bundle);
  }
  assert.equal(advanced.status, NEUROSURGERY_SESSION_TERMINAL_STATUS);
  const finished = await agent.finishPublicLiteratureSession(request, advanced, bundle);
  assert.equal(finished.status, "needs_evidence");
  const run = await agent.runPublicLiteratureSession(request, bundle, {}, 2);
  assert.equal(run.session.status, NEUROSURGERY_SESSION_TERMINAL_STATUS);
  const mission = await agent.runPublicLiteratureMission(
    request,
    bundle,
    {},
    { specialty: "encephalocele", text: "encephalocele", limit: 2 },
    2,
    { as_of: "2027-08-31T00:00:00Z", max_age_days: 30 },
    { specialties: ["encephalocele", "glioma"], max_hits_per_lane: 1, max_review_items_per_lane: 1, max_issues_per_lane: 1 },
  );
  assert.equal(mission.provider, "none");
  assert.equal(
    mission.public_literature_integrity_audit.schema_version,
    "bioprism-neurosurgery-public-literature-integrity-audit/0.1",
  );
  assert.equal(
    mission.public_literature_review_queue.schema_version,
    "bioprism-neurosurgery-public-literature-review-queue/0.1",
  );
  assert.equal(
    mission.public_literature_workbench.schema_version,
    "bioprism-neurosurgery-public-literature-workbench/0.1",
  );
  assert.equal(
    mission.public_literature_portfolio.schema_version,
    "bioprism-neurosurgery-public-literature-portfolio/0.1",
  );
  assert.equal(mission.public_literature_portfolio.specialty_count, 2);
  assert.equal(mission.public_literature_portfolio.total_match_count, 48);
  assert.equal(client.calls.at(-1).args.public_literature.schema_version, bundle.schema_version);
  assert.equal(client.calls.at(-1).args.freshness.as_of, "2027-08-31T00:00:00Z");
});
