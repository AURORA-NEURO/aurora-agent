//! Small local JSON entry point for `bioprism-neurosurgery`.
// The source touch also permits a Windows Application Control relink when a test binary is blocked.

use bioprism_neurosurgery::{
    CaseAssetManifest, CaseAssetManifestQuery, CaseAssetManifestReport, CaseAssetReviewDecision,
    CaseAssetReviewDispositionReport, CaseRequest, DicomCaseImport, DicomCaseImportReport,
    DicomEvidenceWorkflowQuery, DicomEvidenceWorkflowReport, EvidenceAcquisitionQuery,
    EvidenceAcquisitionSession, EvidenceGraphQuery, EvidenceProgramQuery, EvidenceSynthesisQuery,
    FhirCaseImport, FhirCaseImportReport, GliomaMolecularMapQuery, LiteratureLinkAuditQuery,
    NeurosurgicalAgent, NeurosurgicalIntakePortfolioQuery, NeurosurgicalIntakeQuery,
    NeurosurgicalMissionResult, NeurosurgicalResearchBriefQuery, NeurosurgicalSession,
    PublicLiteratureBundle, PublicLiteratureDraftAuditRequest, PublicLiteratureEvidencePacketQuery,
    PublicLiteratureIntegrityAuditQuery, PublicLiteratureMatrixQuery,
    PublicLiteraturePortfolioQuery, PublicLiteratureQuery, PublicLiteratureReasoningContextQuery,
    PublicLiteratureRefreshAuditQuery, PublicLiteratureReviewQueueQuery,
    PublicLiteratureWorkbenchQuery, RealDataAutonomousWorkflowQuery, RealDataCohortLandscapeQuery,
    RealDataCoverageQuery, RealDataDiffQuery, RealDataDraftAuditRequest,
    RealDataEvidencePacketQuery, RealDataFreshnessQuery, RealDataMolecularCoverageQuery,
    RealDataQuery, RealDataReasoningContextQuery, RealDataReconciliationQuery,
    RealDataRefreshAuditQuery, RealDataReviewDecision, RealDataReviewQueueQuery,
    RealDataTrialLandscapeQuery, RealGliomaBundle, MAX_CASE_ASSET_REVIEW_DISPOSITIONS,
    MAX_EVIDENCE_ACQUISITION_ADVANCE_STEPS, MAX_REAL_DATA_REVIEW_DISPOSITIONS,
    MAX_RESEARCH_PLAN_REFERENCES, MAX_RESEARCH_PLAN_TASKS, MAX_SESSION_STEPS,
    NEUROSURGERY_SCHEMA_VERSION,
};
use serde_json::Value;
use std::io::{self, Read};

fn main() {
    // Persisted mission envelopes include several nested source-addressable reports and can be
    // hundreds of kilobytes even when bounded. Run the JSON entry point on an explicit stack so
    // replay validation is reliable on platforms whose process-main stack is comparatively small.
    std::thread::Builder::new()
        .name("neurosurgery-cli".to_string())
        .stack_size(16 * 1024 * 1024)
        .spawn(run)
        .expect("neurosurgery CLI worker thread should start")
        .join()
        .expect("neurosurgery CLI worker thread should finish");
}

fn run() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let agent = NeurosurgicalAgent::default();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "bioprism-neurosurgery {NEUROSURGERY_SCHEMA_VERSION}\n\nRead one CaseRequest JSON document from stdin and emit one AgentResponse JSON document.\n\nOptions:\n  --catalogue                       emit the read-only tool catalogue instead of reading stdin\n  --specialty-catalogue             emit all specialty profiles and tool specs\n  --real-glioma <path>              enrich a glioma research run with a validated public-data bundle\n  --query-real-glioma <path>        read a RealDataQuery JSON document and query a bundle\n  --validate-real-glioma <path>     validate a bundle and print its provenance summary\n  --real-data-hashes <path>         print canonical source hashes for an ingestion job\n  --session-start                   read a request and emit a resumable session checkpoint\n  --session-advance <path>          read a request and checkpoint JSON, then advance one tool\n  --session-finish <path>           read a request and final checkpoint, then emit AgentResponse\n  --session-run                     execute a bounded session to the human-review hold\n  --max-session-steps <n>           cap --session-run at 1..={MAX_SESSION_STEPS} route steps\n  --help                            show this help\n\nThe core performs no network access and needs no API key. The real-data bundle is an explicit local\nfile with source URLs, retrieval metadata, and reproducible SHA-256 content hashes. No clinical\naction, diagnosis, treatment recommendation, triage, or procedural action is emitted."
        );
        println!("  --validate-mission <path>         validate a persisted mission against a request, snapshots, and optional original --mission-case-dicom/--mission-case-fhir imports");
        println!("  --case-fhir-import <path>          import a sanitized real FHIR Bundle metadata document using a CaseRequest on stdin");
        println!("  --case-dicom-import <path>         import sanitized DICOM JSON series metadata using a CaseRequest on stdin");
        println!("  --mission-case-dicom <path>         bind sanitized DICOM JSON metadata directly into a real-data glioma mission");
        println!("  --mission-case-fhir <path>          bind sanitized FHIR metadata directly into a validated mission");
        println!("  --case-dicom-evidence-workflow <path> compose DICOM metadata with real/public evidence into one digest-bound review packet");
        println!("  --case-dicom-evidence-workflow-query <path> optional DICOM-to-evidence workflow query JSON");
        println!("  --intake-mission                  route a natural-language intake query using local evidence bundles");
        println!("  --intake-case-dicom <path>         attach a sanitized DICOM metadata import to --intake-mission");
        println!("  --intake-case-fhir <path>          attach a sanitized FHIR metadata import to --intake-mission");
        println!("  --intake-portfolio                fan out an intake query across one or all specialty lanes");
        println!("  --audit-evidence                  emit the granular specialty intake coverage matrix");
        println!("  --specialty-evidence-map           emit explicit identity/spatial/functional/temporal coverage for one specialty");
        println!("  --temporal-audit                  emit explicit observation date/timepoint alignment coverage");
        println!("  --evidence-graph <path>           read an EvidenceGraphQuery JSON document and graph a bundle");
        println!("  --real-data-coverage <path>       read a RealDataCoverageQuery JSON document and audit a bundle");
        println!("  --real-data-reconciliation <path> reconcile exact PMID/DOI identifiers inside a bundle using a query on stdin");
        println!("  --real-data-freshness <path>       audit source retrieval age against an explicit UTC as_of query on stdin");
        println!("  --diff-real-glioma <before> <after> compare two validated public snapshots using a query on stdin");
        println!("  --real-data-refresh-audit <before> <after> reconcile two validated snapshots into one review report");
        println!("  --real-data-refresh-audit-query <path> optional refresh-audit query JSON (request remains on stdin)");
        println!("  --real-data-review-queue <path>   derive bounded metadata-review tasks from a bundle using a query on stdin");
        println!("  --real-data-review-disposition <path> apply human metadata-review decisions to a queue using a JSON array on stdin");
        println!("  --real-data-evidence-packet <path> compose summary, coverage, trial/cohort landscapes, graph, query, and review queue from a bundle using a packet query on stdin");
        println!("  --real-data-autonomous-workflow <path> compose a resumable source-bound metadata review wave from a bundle using a workflow query on stdin");
        println!("  --real-data-reasoning-context <path> render a bounded source-addressable local-model context from a bundle using a context query on stdin");
        println!("  --real-data-draft-audit <path>     audit local-model/reviewer claims against a real-data packet using a draft JSON object on stdin");
        println!("  --real-data-trial-landscape <path> summarize bounded ClinicalTrials.gov metadata from a real glioma bundle using a query on stdin");
        println!("  --real-data-molecular-coverage <path> inventory bounded cBioPortal assay/profile and aggregate GDC file data-type metadata from a real glioma bundle using a query on stdin");
        println!("  --real-data-cohort-landscape <path> compare source-linked TCGA/GDC project metadata and aggregate file/data-type availability using a query on stdin");
        println!("  --public-literature-evidence-packet <path> compose a bounded PMID-backed packet from a public-literature bundle using a packet query on stdin");
        println!("  --public-literature-reasoning-context <path> render a bounded PMID/source-addressable local-model context from a public-literature bundle using a context query on stdin");
        println!("  --public-literature-draft-audit <path> audit local-model/reviewer claims against emitted PMIDs using a draft JSON object on stdin");
        println!("  --public-literature-matrix <path> fan out a bounded real-literature query across selected specialty lanes using a matrix JSON document on stdin");
        println!("  --public-literature-freshness <path> audit public-literature source retrieval age against an explicit UTC as_of query on stdin");
        println!("  --public-literature-refresh-audit <before> <after> reconcile two validated literature snapshots using an audit query on stdin");
        println!("  --literature-link-audit <real> <public> link a real glioma literature index to a public-literature lane by exact PMID/DOI identifiers");
        println!("  --public-literature-integrity-audit <path> audit real PubMed source/record completeness and identifier hygiene");
        println!("  --public-literature-review-queue <path> derive bounded source-linked reviewer tasks from real PubMed integrity findings using a query on stdin");
        println!("  --public-literature-workbench <path> join specialty profiles to real PubMed coverage and review obligations using a query on stdin");
        println!("  --public-literature-portfolio <path> run a bounded autonomous multi-lane PubMed pass with per-lane queries and review queues");
        println!("  --evidence-program            build source-grounded specialty review tracks from validated snapshots");
        println!("  --evidence-program-query <path> optional evidence-program query JSON");
        println!("  --research-brief                 extract a deterministic topic brief from one validated bundle");
        println!(
            "  --research-brief-query <path>    optional research-brief extraction query JSON"
        );
        println!("  --autonomous-acquisition         with --research-plan, emit bounded local queries across supplied real-data planes");
        println!("  --autonomous-acquisition-query <path> optional EvidenceAcquisitionQuery JSON");
        println!("  --autonomous-acquisition-operation <compile|start|advance|finish> lifecycle operation (default compile)");
        println!("  --autonomous-acquisition-session <path> checkpoint JSON for advance/finish");
        println!("  --autonomous-acquisition-max-steps <n> replay bound for advance (1..={MAX_EVIDENCE_ACQUISITION_ADVANCE_STEPS})");
        println!("  --autonomous-acquisition-case-asset-review-disposition <path> persisted disposition report for the acquisition lifecycle");
        println!("  --research-plan                  compile bounded source-linked research tasks from intake gaps");
        println!(
            "  --research-plan-max-tasks <n>    cap planner tasks at 1..={MAX_RESEARCH_PLAN_TASKS}"
        );
        println!("  --research-plan-max-references <n> cap source references per task at 1..={MAX_RESEARCH_PLAN_REFERENCES}");
        println!("  --evidence-synthesis              align a de-identified request with one or both validated public evidence bundles");
        println!("  --evidence-synthesis-query <path> optional evidence-synthesis query JSON");
        println!("  --glioma-molecular-map            ground typed glioma markers against validated real/Public snapshots");
        println!("  --glioma-molecular-map-query <path> optional marker-map query JSON");
        println!("  --case-asset-manifest <path>       project or attach a real de-identified multimodal asset manifest (metadata only; with --evidence-program/--evidence-synthesis/--autonomous-acquisition/--mission/--intake-mission/selected --intake-portfolio it is included in the evidence envelope)");
        println!("  --case-asset-manifest-query <path> optional manifest projection query JSON (used with --case-asset-manifest)");
        println!("  --case-asset-review-disposition <path> apply reviewer decisions from stdin to a persisted case-asset report");
        println!("  --mission-case-asset-review-disposition <path> attach a persisted case-asset disposition ledger to --mission");
        println!("  --intake-case-asset-review-disposition <path> attach a persisted case-asset disposition ledger to --intake-mission/--intake-portfolio");
        println!(
            "\nPublic-literature options:\n  --public-literature <path>        enrich any supported specialty with a validated PubMed snapshot (may accompany --real-glioma for missions)\n  --query-public-literature <path>  read a PublicLiteratureQuery JSON document and query a snapshot\n  --validate-public-literature <path> validate a snapshot and print its provenance summary\n  --public-literature-hashes <path> print canonical snapshot source hashes\n\nMission options:\n  --mission                         compose discovery, optional query, and session to human review\n  --mission-query <path>            query JSON file used by --mission (real-side query in dual-bundle mode)\n  --mission-public-literature-query <path> query JSON file for the PubMed side in dual-bundle mode\n  --mission-portfolio-query <path>  optional multi-lane portfolio query JSON for a public-literature mission\n  --mission-freshness <path>        explicit UTC source-age query JSON for --mission"
        );
        println!("  --intake-freshness <path>        explicit UTC source-age query JSON for --intake-mission/--intake-portfolio");
        return;
    }
    if args.iter().any(|arg| arg == "--catalogue") {
        println!(
            "{}",
            serde_json::to_string_pretty(&agent.catalogue()).expect("catalogue is serialisable")
        );
        return;
    }
    if args.iter().any(|arg| arg == "--specialty-catalogue") {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": NEUROSURGERY_SCHEMA_VERSION,
                "specialties": agent.specialty_profiles(),
                "tools": agent.catalogue(),
                "provider": "none",
                "network": false,
                "effects": ["read_only"],
            }))
            .expect("specialty catalogue is serialisable")
        );
        return;
    }
    if let Some(path) = args
        .windows(2)
        .find(|pair| pair[0] == "--validate-real-glioma")
        .map(|pair| pair[1].clone())
    {
        let data_text = match std::fs::read_to_string(&path) {
            Ok(data_text) => data_text,
            Err(error) => {
                emit_error(&format!(
                    "could not read real glioma data bundle {path:?}: {error}"
                ));
                std::process::exit(2);
            }
        };
        let data: RealGliomaBundle = match serde_json::from_str(&data_text) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&format!("invalid real glioma data JSON: {error}"));
                std::process::exit(2);
            }
        };
        match data.summary() {
            Ok(summary) => println!(
                "{}",
                serde_json::to_string_pretty(&summary).expect("summary is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }
    if let Some(path) = args
        .windows(2)
        .find(|pair| pair[0] == "--validate-public-literature")
        .map(|pair| pair[1].clone())
    {
        let data = match read_public_literature(&path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        match data.summary() {
            Ok(summary) => println!(
                "{}",
                serde_json::to_string_pretty(&summary).expect("summary is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    let mission = args.iter().any(|arg| arg == "--mission");
    let temporal_audit = args.iter().any(|arg| arg == "--temporal-audit");
    let audit_evidence = args.iter().any(|arg| arg == "--audit-evidence") || temporal_audit;
    let specialty_evidence_map = args.iter().any(|arg| arg == "--specialty-evidence-map");
    let evidence_synthesis = args.iter().any(|arg| arg == "--evidence-synthesis");
    let glioma_molecular_map = args.iter().any(|arg| arg == "--glioma-molecular-map");
    let case_asset_manifest_path = argument_value(&args, "--case-asset-manifest");
    let case_fhir_import_path = argument_value(&args, "--case-fhir-import");
    let case_dicom_import_path = argument_value(&args, "--case-dicom-import");
    let case_dicom_evidence_workflow_path = argument_value(&args, "--case-dicom-evidence-workflow");
    let case_dicom_evidence_workflow_query_path =
        argument_value(&args, "--case-dicom-evidence-workflow-query");
    let research_plan = args.iter().any(|arg| arg == "--research-plan");
    let autonomous_acquisition = args.iter().any(|arg| arg == "--autonomous-acquisition");
    let evidence_graph_path = argument_value(&args, "--evidence-graph");
    let real_data_coverage_path = argument_value(&args, "--real-data-coverage");
    let real_data_reconciliation_path = argument_value(&args, "--real-data-reconciliation");
    let real_data_freshness_path = argument_value(&args, "--real-data-freshness");
    let real_data_diff_paths = argument_pair(&args, "--diff-real-glioma");
    let real_data_refresh_audit_paths = argument_pair(&args, "--real-data-refresh-audit");
    let real_data_refresh_audit_query_path =
        argument_value(&args, "--real-data-refresh-audit-query");
    let real_data_review_queue_path = argument_value(&args, "--real-data-review-queue");
    let real_data_review_disposition_path = argument_value(&args, "--real-data-review-disposition");
    let real_data_evidence_packet_path = argument_value(&args, "--real-data-evidence-packet");
    let real_data_autonomous_workflow_path =
        argument_value(&args, "--real-data-autonomous-workflow");
    let real_data_reasoning_context_path = argument_value(&args, "--real-data-reasoning-context");
    let real_data_draft_audit_path = argument_value(&args, "--real-data-draft-audit");
    let real_data_trial_landscape_path = argument_value(&args, "--real-data-trial-landscape");
    let real_data_molecular_coverage_path = argument_value(&args, "--real-data-molecular-coverage");
    let real_data_cohort_landscape_path = argument_value(&args, "--real-data-cohort-landscape");
    let public_literature_evidence_packet_path =
        argument_value(&args, "--public-literature-evidence-packet");
    let public_literature_reasoning_context_path =
        argument_value(&args, "--public-literature-reasoning-context");
    let public_literature_draft_audit_path =
        argument_value(&args, "--public-literature-draft-audit");
    let public_literature_matrix_path = argument_value(&args, "--public-literature-matrix");
    let public_literature_freshness_path = argument_value(&args, "--public-literature-freshness");
    let public_literature_refresh_audit_paths =
        argument_pair(&args, "--public-literature-refresh-audit");
    let literature_link_audit_paths = argument_pair(&args, "--literature-link-audit");
    let public_literature_integrity_audit_path =
        argument_value(&args, "--public-literature-integrity-audit");
    let public_literature_review_queue_path =
        argument_value(&args, "--public-literature-review-queue");
    let public_literature_workbench_path = argument_value(&args, "--public-literature-workbench");
    let public_literature_portfolio_path = argument_value(&args, "--public-literature-portfolio");
    let evidence_program = args.iter().any(|arg| arg == "--evidence-program");
    let evidence_program_query_path = argument_value(&args, "--evidence-program-query");
    let research_brief = args.iter().any(|arg| arg == "--research-brief");
    let research_brief_query_path = argument_value(&args, "--research-brief-query");
    let autonomous_acquisition_query_path = argument_value(&args, "--autonomous-acquisition-query");
    let autonomous_acquisition_operation_arg =
        argument_value(&args, "--autonomous-acquisition-operation");
    let autonomous_acquisition_session_path =
        argument_value(&args, "--autonomous-acquisition-session");
    let autonomous_acquisition_max_steps_arg =
        argument_value(&args, "--autonomous-acquisition-max-steps");
    let autonomous_acquisition_case_asset_disposition_path = argument_value(
        &args,
        "--autonomous-acquisition-case-asset-review-disposition",
    );
    let evidence_synthesis_query_path = argument_value(&args, "--evidence-synthesis-query");
    let glioma_molecular_map_query_path = argument_value(&args, "--glioma-molecular-map-query");
    let case_asset_manifest_query_path = argument_value(&args, "--case-asset-manifest-query");
    let case_asset_review_disposition_path =
        argument_value(&args, "--case-asset-review-disposition");
    let intake_case_asset_review_disposition_path =
        argument_value(&args, "--intake-case-asset-review-disposition");
    let mission_case_asset_review_disposition_path =
        argument_value(&args, "--mission-case-asset-review-disposition");
    let mission_case_dicom_path = argument_value(&args, "--mission-case-dicom");
    let mission_case_fhir_path = argument_value(&args, "--mission-case-fhir");
    let mission_query_path = argument_value(&args, "--mission-query");
    let mission_public_literature_query_path =
        argument_value(&args, "--mission-public-literature-query");
    let mission_portfolio_query_path = argument_value(&args, "--mission-portfolio-query");
    let mission_freshness_path = argument_value(&args, "--mission-freshness");
    let intake_freshness_path = argument_value(&args, "--intake-freshness");
    let intake_case_dicom_path = argument_value(&args, "--intake-case-dicom");
    let intake_case_fhir_path = argument_value(&args, "--intake-case-fhir");
    let mission_validation_path = argument_value(&args, "--validate-mission");
    let session_start = args.iter().any(|arg| arg == "--session-start");
    let session_advance = argument_value(&args, "--session-advance");
    let session_finish = argument_value(&args, "--session-finish");
    let session_run = args.iter().any(|arg| arg == "--session-run");
    let intake_mission = args.iter().any(|arg| arg == "--intake-mission");
    let intake_portfolio = args.iter().any(|arg| arg == "--intake-portfolio");
    let case_asset_manifest_mode = case_asset_manifest_path.is_some()
        && !mission
        && !intake_mission
        && !intake_portfolio
        && !evidence_synthesis
        && !evidence_program
        && !(research_plan && autonomous_acquisition);
    let case_fhir_import_mode = case_fhir_import_path.is_some();
    let case_dicom_import_mode = case_dicom_import_path.is_some();
    let case_dicom_evidence_workflow_mode = case_dicom_evidence_workflow_path.is_some();
    let max_session_steps_arg = argument_value(&args, "--max-session-steps");
    let max_research_plan_tasks_arg = argument_value(&args, "--research-plan-max-tasks");
    let max_research_plan_references_arg = argument_value(&args, "--research-plan-max-references");
    let research_plan_bounds_requested =
        max_research_plan_tasks_arg.is_some() || max_research_plan_references_arg.is_some();
    if session_start as u8
        + session_advance.is_some() as u8
        + session_finish.is_some() as u8
        + session_run as u8
        + mission as u8
        + intake_mission as u8
        + intake_portfolio as u8
        + audit_evidence as u8
        + specialty_evidence_map as u8
        + evidence_synthesis as u8
        + glioma_molecular_map as u8
        + case_asset_manifest_mode as u8
        + case_fhir_import_mode as u8
        + case_dicom_import_mode as u8
        + case_dicom_evidence_workflow_mode as u8
        + case_asset_review_disposition_path.is_some() as u8
        + research_plan as u8
        + evidence_graph_path.is_some() as u8
        + real_data_coverage_path.is_some() as u8
        + real_data_reconciliation_path.is_some() as u8
        + real_data_freshness_path.is_some() as u8
        + real_data_diff_paths.is_some() as u8
        + real_data_refresh_audit_paths.is_some() as u8
        + real_data_review_queue_path.is_some() as u8
        + real_data_review_disposition_path.is_some() as u8
        + real_data_evidence_packet_path.is_some() as u8
        + real_data_autonomous_workflow_path.is_some() as u8
        + real_data_reasoning_context_path.is_some() as u8
        + real_data_draft_audit_path.is_some() as u8
        + real_data_trial_landscape_path.is_some() as u8
        + real_data_molecular_coverage_path.is_some() as u8
        + real_data_cohort_landscape_path.is_some() as u8
        + public_literature_evidence_packet_path.is_some() as u8
        + public_literature_reasoning_context_path.is_some() as u8
        + public_literature_draft_audit_path.is_some() as u8
        + public_literature_matrix_path.is_some() as u8
        + public_literature_freshness_path.is_some() as u8
        + public_literature_refresh_audit_paths.is_some() as u8
        + literature_link_audit_paths.is_some() as u8
        + public_literature_integrity_audit_path.is_some() as u8
        + public_literature_review_queue_path.is_some() as u8
        + public_literature_workbench_path.is_some() as u8
        + public_literature_portfolio_path.is_some() as u8
        + evidence_program as u8
        + research_brief as u8
        + mission_validation_path.is_some() as u8
        > 1
    {
        emit_error("run modes are mutually exclusive");
        std::process::exit(2);
    }
    if research_brief_query_path.is_some() && !research_brief {
        emit_error("--research-brief-query can only be used with --research-brief");
        std::process::exit(2);
    }
    if evidence_program_query_path.is_some() && !evidence_program {
        emit_error("--evidence-program-query can only be used with --evidence-program");
        std::process::exit(2);
    }
    if case_dicom_evidence_workflow_query_path.is_some() && !case_dicom_evidence_workflow_mode {
        emit_error(
            "--case-dicom-evidence-workflow-query can only be used with --case-dicom-evidence-workflow",
        );
        std::process::exit(2);
    }
    if autonomous_acquisition && !research_plan {
        emit_error("--autonomous-acquisition can only be used with --research-plan");
        std::process::exit(2);
    }
    if autonomous_acquisition_query_path.is_some() && !autonomous_acquisition {
        emit_error("--autonomous-acquisition-query can only be used with --autonomous-acquisition");
        std::process::exit(2);
    }
    let autonomous_acquisition_operation = autonomous_acquisition_operation_arg
        .as_deref()
        .unwrap_or("compile");
    if !matches!(
        autonomous_acquisition_operation,
        "compile" | "start" | "advance" | "finish"
    ) {
        emit_error("--autonomous-acquisition-operation must be compile, start, advance, or finish");
        std::process::exit(2);
    }
    if autonomous_acquisition_operation_arg.is_some() && !autonomous_acquisition {
        emit_error(
            "--autonomous-acquisition-operation can only be used with --autonomous-acquisition",
        );
        std::process::exit(2);
    }
    if autonomous_acquisition_session_path.is_some() && !autonomous_acquisition {
        emit_error(
            "--autonomous-acquisition-session can only be used with --autonomous-acquisition",
        );
        std::process::exit(2);
    }
    if matches!(autonomous_acquisition_operation, "advance" | "finish")
        && autonomous_acquisition_session_path.is_none()
    {
        emit_error(
            "--autonomous-acquisition-session is required for advance and finish operations",
        );
        std::process::exit(2);
    }
    if !matches!(autonomous_acquisition_operation, "advance" | "finish")
        && autonomous_acquisition_session_path.is_some()
    {
        emit_error(
            "--autonomous-acquisition-session is only valid for advance and finish operations",
        );
        std::process::exit(2);
    }
    let autonomous_acquisition_max_steps = match autonomous_acquisition_max_steps_arg.as_deref() {
        Some(value) => match value.parse::<usize>() {
            Ok(value) if (1..=MAX_EVIDENCE_ACQUISITION_ADVANCE_STEPS).contains(&value) => value,
            _ => {
                emit_error(&format!(
                    "--autonomous-acquisition-max-steps must be between 1 and {MAX_EVIDENCE_ACQUISITION_ADVANCE_STEPS}"
                ));
                std::process::exit(2);
            }
        },
        None => 1,
    };
    if autonomous_acquisition_max_steps_arg.is_some() && !autonomous_acquisition {
        emit_error(
            "--autonomous-acquisition-max-steps can only be used with --autonomous-acquisition",
        );
        std::process::exit(2);
    }
    if autonomous_acquisition_max_steps_arg.is_some()
        && autonomous_acquisition_operation != "advance"
    {
        emit_error("--autonomous-acquisition-max-steps is only valid for the advance operation");
        std::process::exit(2);
    }
    if autonomous_acquisition_case_asset_disposition_path.is_some() && !autonomous_acquisition {
        emit_error(
            "--autonomous-acquisition-case-asset-review-disposition can only be used with --autonomous-acquisition",
        );
        std::process::exit(2);
    }
    if autonomous_acquisition_case_asset_disposition_path.is_some()
        && case_asset_manifest_path.is_none()
    {
        emit_error(
            "--autonomous-acquisition-case-asset-review-disposition requires --case-asset-manifest",
        );
        std::process::exit(2);
    }
    if evidence_synthesis_query_path.is_some() && !evidence_synthesis {
        emit_error("--evidence-synthesis-query can only be used with --evidence-synthesis");
        std::process::exit(2);
    }
    if glioma_molecular_map_query_path.is_some() && !glioma_molecular_map {
        emit_error("--glioma-molecular-map-query can only be used with --glioma-molecular-map");
        std::process::exit(2);
    }
    if case_asset_manifest_query_path.is_some() && case_asset_manifest_path.is_none() {
        emit_error("--case-asset-manifest-query can only be used with --case-asset-manifest");
        std::process::exit(2);
    }
    if case_asset_review_disposition_path.is_some() && case_asset_manifest_query_path.is_some() {
        emit_error(
            "--case-asset-review-disposition cannot be combined with --case-asset-manifest-query",
        );
        std::process::exit(2);
    }
    if intake_case_asset_review_disposition_path.is_some() && !intake_mission && !intake_portfolio {
        emit_error(
            "--intake-case-asset-review-disposition can only be used with --intake-mission or --intake-portfolio",
        );
        std::process::exit(2);
    }
    if intake_case_asset_review_disposition_path.is_some() && case_asset_manifest_path.is_none() {
        emit_error("--intake-case-asset-review-disposition requires --case-asset-manifest");
        std::process::exit(2);
    }
    if mission_case_asset_review_disposition_path.is_some() && !mission {
        emit_error("--mission-case-asset-review-disposition can only be used with --mission");
        std::process::exit(2);
    }
    if real_data_refresh_audit_query_path.is_some() && real_data_refresh_audit_paths.is_none() {
        emit_error(
            "--real-data-refresh-audit-query can only be used with --real-data-refresh-audit",
        );
        std::process::exit(2);
    }
    let max_research_plan_tasks = match max_research_plan_tasks_arg {
        Some(value) => match value.parse::<usize>() {
            Ok(value) if (1..=MAX_RESEARCH_PLAN_TASKS).contains(&value) => value,
            _ => {
                emit_error(&format!(
                    "--research-plan-max-tasks must be between 1 and {MAX_RESEARCH_PLAN_TASKS}"
                ));
                std::process::exit(2);
            }
        },
        None => MAX_RESEARCH_PLAN_TASKS.min(8),
    };
    let max_research_plan_references = match max_research_plan_references_arg {
        Some(value) => match value.parse::<usize>() {
            Ok(value) if (1..=MAX_RESEARCH_PLAN_REFERENCES).contains(&value) => value,
            _ => {
                emit_error(&format!(
                    "--research-plan-max-references must be between 1 and {MAX_RESEARCH_PLAN_REFERENCES}"
                ));
                std::process::exit(2);
            }
        },
        None => MAX_RESEARCH_PLAN_REFERENCES.min(4),
    };
    if research_plan_bounds_requested && !research_plan {
        emit_error("research-plan bounds can only be used with --research-plan");
        std::process::exit(2);
    }
    let max_session_steps = match max_session_steps_arg.as_deref() {
        Some(value) => match value.parse::<usize>() {
            Ok(value) if (1..=MAX_SESSION_STEPS).contains(&value) => value,
            _ => {
                emit_error(&format!(
                    "--max-session-steps must be between 1 and {MAX_SESSION_STEPS}"
                ));
                std::process::exit(2);
            }
        },
        None => MAX_SESSION_STEPS,
    };
    if max_session_steps != MAX_SESSION_STEPS
        && !session_run
        && !mission
        && !intake_mission
        && !intake_portfolio
        && !evidence_synthesis
        && !glioma_molecular_map
    {
        emit_error(
            "--max-session-steps can only be used with --session-run, --mission, --intake-mission, or --intake-portfolio",
        );
        std::process::exit(2);
    }
    if mission_query_path.is_some() && !mission {
        emit_error("--mission-query can only be used with --mission");
        std::process::exit(2);
    }
    if mission_case_dicom_path.is_some() && !mission && mission_validation_path.is_none() {
        emit_error("--mission-case-dicom can only be used with --mission");
        std::process::exit(2);
    }
    if mission_case_fhir_path.is_some() && !mission && mission_validation_path.is_none() {
        emit_error("--mission-case-fhir can only be used with --mission");
        std::process::exit(2);
    }
    if mission_public_literature_query_path.is_some() && !mission {
        emit_error("--mission-public-literature-query can only be used with --mission");
        std::process::exit(2);
    }
    if mission_portfolio_query_path.is_some() && !mission {
        emit_error("--mission-portfolio-query can only be used with --mission");
        std::process::exit(2);
    }
    if mission_freshness_path.is_some() && !mission {
        emit_error("--mission-freshness can only be used with --mission");
        std::process::exit(2);
    }
    if intake_freshness_path.is_some() && !intake_mission && !intake_portfolio {
        emit_error(
            "--intake-freshness can only be used with --intake-mission or --intake-portfolio",
        );
        std::process::exit(2);
    }
    if intake_case_dicom_path.is_some() || intake_case_fhir_path.is_some() {
        if !intake_mission {
            emit_error(
                "--intake-case-dicom/--intake-case-fhir can only be used with --intake-mission",
            );
            std::process::exit(2);
        }
        if intake_case_dicom_path.is_some() && case_asset_manifest_path.is_some()
            || intake_case_fhir_path.is_some() && case_asset_manifest_path.is_some()
        {
            emit_error(
                "--intake-case-dicom/--intake-case-fhir cannot be combined with --case-asset-manifest",
            );
            std::process::exit(2);
        }
        if case_asset_manifest_query_path.is_some() {
            emit_error(
                "--intake-case-dicom/--intake-case-fhir cannot be combined with --case-asset-manifest-query",
            );
            std::process::exit(2);
        }
        if intake_case_asset_review_disposition_path.is_some() {
            emit_error(
                "--intake-case-asset-review-disposition cannot be combined with --intake-case-dicom/--intake-case-fhir",
            );
            std::process::exit(2);
        }
    }
    if let Some(path) = args
        .windows(2)
        .find(|pair| pair[0] == "--real-data-hashes")
        .map(|pair| pair[1].clone())
    {
        let data_text = match std::fs::read_to_string(&path) {
            Ok(data_text) => data_text,
            Err(error) => {
                emit_error(&format!(
                    "could not read real glioma data bundle {path:?}: {error}"
                ));
                std::process::exit(2);
            }
        };
        let data: RealGliomaBundle = match serde_json::from_str(&data_text) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&format!("invalid real glioma data JSON: {error}"));
                std::process::exit(2);
            }
        };
        match data.canonical_source_hashes() {
            Ok(hashes) => println!(
                "{}",
                serde_json::to_string_pretty(&hashes).expect("hash map is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some(path) = args
        .windows(2)
        .find(|pair| pair[0] == "--public-literature-hashes")
        .map(|pair| pair[1].clone())
    {
        let data = match read_public_literature(&path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        match data.canonical_source_hashes() {
            Ok(hashes) => println!(
                "{}",
                serde_json::to_string_pretty(&hashes).expect("hash map is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some(path) = case_asset_review_disposition_path.as_deref() {
        let report_text = match std::fs::read_to_string(path) {
            Ok(report_text) => report_text,
            Err(error) => {
                emit_error(&format!(
                    "could not read case-asset review report {path:?}: {error}"
                ));
                std::process::exit(2);
            }
        };
        let report: CaseAssetManifestReport = match serde_json::from_str(&report_text) {
            Ok(report) => report,
            Err(error) => {
                emit_error(&format!("invalid case-asset review report JSON: {error}"));
                std::process::exit(2);
            }
        };
        let mut decisions_text = String::new();
        if let Err(error) = io::stdin().read_to_string(&mut decisions_text) {
            emit_error(&format!(
                "could not read case-asset review decisions from stdin: {error}"
            ));
            std::process::exit(2);
        }
        let decisions: Vec<CaseAssetReviewDecision> = match serde_json::from_str(&decisions_text) {
            Ok(decisions) => decisions,
            Err(error) => {
                emit_error(&format!(
                    "invalid case-asset review decisions JSON: {error}"
                ));
                std::process::exit(2);
            }
        };
        if decisions.len() > MAX_CASE_ASSET_REVIEW_DISPOSITIONS {
            emit_error(&format!(
                "case-asset review decisions exceed {MAX_CASE_ASSET_REVIEW_DISPOSITIONS} items"
            ));
            std::process::exit(2);
        }
        match agent.case_asset_review_disposition(&report, &decisions) {
            Ok(result) => println!(
                "{}",
                serde_json::to_string_pretty(&result)
                    .expect("case-asset disposition is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    let mut input = String::new();
    if let Err(error) = io::stdin().read_to_string(&mut input) {
        emit_error(&format!("could not read stdin: {error}"));
        std::process::exit(2);
    }
    let document: Value = match serde_json::from_str(&input) {
        Ok(document) => document,
        Err(error) => {
            emit_error(&format!("invalid request JSON: {error}"));
            std::process::exit(2);
        }
    };
    let real_data_path = args
        .windows(2)
        .find(|pair| pair[0] == "--real-glioma")
        .map(|pair| pair[1].clone());
    let public_literature_path = argument_value(&args, "--public-literature");
    if let Some(mission_path) = mission_validation_path.as_deref() {
        if args.iter().any(|arg| {
            matches!(
                arg.as_str(),
                "--mission"
                    | "--session-start"
                    | "--session-run"
                    | "--intake-mission"
                    | "--intake-portfolio"
                    | "--audit-evidence"
                    | "--temporal-audit"
                    | "--evidence-synthesis"
                    | "--evidence-program"
                    | "--research-plan"
                    | "--query-real-glioma"
                    | "--query-public-literature"
            )
        }) {
            emit_error(
                "--validate-mission accepts only the mission path, a request on stdin, and optional snapshots",
            );
            std::process::exit(2);
        }
        let mission_text = match std::fs::read_to_string(mission_path) {
            Ok(text) => text,
            Err(error) => {
                emit_error(&format!(
                    "could not read persisted mission {mission_path:?}: {error}"
                ));
                std::process::exit(2);
            }
        };
        let mission: NeurosurgicalMissionResult = match serde_json::from_str(&mission_text) {
            Ok(mission) => mission,
            Err(error) => {
                emit_error(&format!("invalid persisted mission JSON: {error}"));
                std::process::exit(2);
            }
        };
        let request: CaseRequest = match serde_json::from_value(document.clone()) {
            Ok(request) => request,
            Err(error) => {
                emit_error(&format!("invalid neurosurgical request JSON: {error}"));
                std::process::exit(2);
            }
        };
        let real_data = match real_data_path.as_deref() {
            Some(path) => match read_real_data(path) {
                Ok(data) => Some(data),
                Err(error) => {
                    emit_error(&error);
                    std::process::exit(2);
                }
            },
            None => None,
        };
        let public_literature = match public_literature_path.as_deref() {
            Some(path) => match read_public_literature(path) {
                Ok(data) => Some(data),
                Err(error) => {
                    emit_error(&error);
                    std::process::exit(2);
                }
            },
            None => None,
        };
        let case_dicom_import = match mission_case_dicom_path.as_deref() {
            Some(path) => match read_dicom_case_import(path) {
                Ok(import) => Some(import),
                Err(error) => {
                    emit_error(&error);
                    std::process::exit(2);
                }
            },
            None => None,
        };
        let case_fhir_import = match mission_case_fhir_path.as_deref() {
            Some(path) => match read_fhir_case_import(path) {
                Ok(import) => Some(import),
                Err(error) => {
                    emit_error(&error);
                    std::process::exit(2);
                }
            },
            None => None,
        };
        match mission.validate_for_inputs_with_case_imports(
            &request,
            real_data.as_ref(),
            public_literature.as_ref(),
            case_dicom_import.as_ref(),
            case_fhir_import.as_ref(),
        ) {
            Ok(()) => {
                let audit_digest = mission
                    .mission_audit
                    .as_ref()
                    .map(|audit| audit.audit_digest.clone());
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "valid": true,
                        "mission_id": mission.mission_id,
                        "specialty": mission.specialty,
                        "status": mission.status,
                        "human_review_required": mission.human_review_required,
                        "request_digest": mission.run.response.request_digest,
                        "audit_digest": audit_digest,
                        "provider": mission.provider,
                        "network": mission.network,
                    }))
                    .expect("mission validation result is serialisable")
                );
            }
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }
    if real_data_path.is_some()
        && public_literature_path.is_some()
        && !mission
        && !intake_mission
        && !intake_portfolio
        && !evidence_synthesis
        && !glioma_molecular_map
        && !evidence_program
        && !(research_plan && autonomous_acquisition)
        && !case_dicom_evidence_workflow_mode
    {
        emit_error("--real-glioma and --public-literature are mutually exclusive evidence bundles");
        std::process::exit(2);
    }
    if audit_evidence && (real_data_path.is_some() || public_literature_path.is_some()) {
        emit_error("--audit-evidence cannot be combined with an evidence bundle; it audits request intake only");
        std::process::exit(2);
    }
    let query_data_path = argument_value(&args, "--query-real-glioma");
    let public_query_data_path = argument_value(&args, "--query-public-literature");
    if specialty_evidence_map
        && (real_data_path.is_some()
            || public_literature_path.is_some()
            || query_data_path.is_some()
            || public_query_data_path.is_some())
    {
        emit_error(
            "--specialty-evidence-map cannot be combined with an evidence bundle or bundle query; it audits request coverage only",
        );
        std::process::exit(2);
    }
    if (intake_mission || intake_portfolio)
        && (query_data_path.is_some() || public_query_data_path.is_some())
    {
        emit_error(
            "intake modes cannot be combined with --query-real-glioma or --query-public-literature",
        );
        std::process::exit(2);
    }
    if case_asset_manifest_mode
        && (real_data_path.is_some()
            || public_literature_path.is_some()
            || query_data_path.is_some()
            || public_query_data_path.is_some()
            || evidence_graph_path.is_some()
            || real_data_coverage_path.is_some()
            || real_data_freshness_path.is_some()
            || real_data_diff_paths.is_some()
            || real_data_refresh_audit_paths.is_some()
            || real_data_review_queue_path.is_some()
            || real_data_review_disposition_path.is_some()
            || real_data_evidence_packet_path.is_some()
            || real_data_reasoning_context_path.is_some()
            || real_data_draft_audit_path.is_some()
            || public_literature_evidence_packet_path.is_some()
            || public_literature_reasoning_context_path.is_some()
            || public_literature_draft_audit_path.is_some()
            || public_literature_matrix_path.is_some()
            || public_literature_freshness_path.is_some()
            || public_literature_refresh_audit_paths.is_some()
            || literature_link_audit_paths.is_some()
            || public_literature_integrity_audit_path.is_some()
            || public_literature_review_queue_path.is_some()
            || public_literature_workbench_path.is_some()
            || public_literature_portfolio_path.is_some()
            || research_brief
            || research_plan
            || audit_evidence
            || glioma_molecular_map
            || session_start
            || session_advance.is_some()
            || session_finish.is_some()
            || session_run)
    {
        emit_error(
            "--case-asset-manifest cannot be combined with another standalone run mode or evidence bundle",
        );
        std::process::exit(2);
    }

    if let Some((before_path, after_path)) = real_data_diff_paths.as_ref() {
        if real_data_path.is_some()
            || public_literature_path.is_some()
            || query_data_path.is_some()
            || public_query_data_path.is_some()
            || evidence_graph_path.is_some()
            || real_data_coverage_path.is_some()
            || real_data_review_queue_path.is_some()
            || real_data_evidence_packet_path.is_some()
            || real_data_draft_audit_path.is_some()
            || real_data_review_disposition_path.is_some()
            || session_start
            || session_advance.is_some()
            || session_finish.is_some()
            || session_run
            || mission
            || research_plan
            || audit_evidence
        {
            emit_error(
                "--diff-real-glioma accepts only two bundle paths and a RealDataDiffQuery on stdin",
            );
            std::process::exit(2);
        }
        let before = match read_real_data(before_path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let after = match read_real_data(after_path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let query: RealDataDiffQuery = match serde_json::from_value(document) {
            Ok(query) => query,
            Err(error) => {
                emit_error(&format!("invalid real-data diff query JSON: {error}"));
                std::process::exit(2);
            }
        };
        match agent.real_data_diff(&before, &after, &query) {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("real-data diff report is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some((before_path, after_path)) = real_data_refresh_audit_paths.as_ref() {
        if real_data_path.is_some()
            || public_literature_path.is_some()
            || query_data_path.is_some()
            || public_query_data_path.is_some()
            || evidence_graph_path.is_some()
            || real_data_coverage_path.is_some()
            || real_data_freshness_path.is_some()
            || real_data_diff_paths.is_some()
            || real_data_review_queue_path.is_some()
            || real_data_review_disposition_path.is_some()
            || real_data_evidence_packet_path.is_some()
            || real_data_reasoning_context_path.is_some()
            || real_data_draft_audit_path.is_some()
            || public_literature_evidence_packet_path.is_some()
            || public_literature_reasoning_context_path.is_some()
            || public_literature_draft_audit_path.is_some()
            || public_literature_matrix_path.is_some()
            || public_literature_freshness_path.is_some()
            || session_start
            || session_advance.is_some()
            || session_finish.is_some()
            || session_run
            || mission
            || research_plan
            || research_brief
            || audit_evidence
        {
            emit_error(
                "--real-data-refresh-audit accepts only two bundle paths, a request on stdin, and an optional refresh query file",
            );
            std::process::exit(2);
        }
        let before = match read_real_data(before_path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let after = match read_real_data(after_path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let request: CaseRequest = match serde_json::from_value(document.clone()) {
            Ok(request) => request,
            Err(error) => {
                emit_error(&format!(
                    "invalid neurosurgical refresh-audit request JSON: {error}"
                ));
                std::process::exit(2);
            }
        };
        let query_document = match real_data_refresh_audit_query_path.as_deref() {
            Some(path) => match read_query_value(path) {
                Ok(document) => document,
                Err(error) => {
                    emit_error(&error);
                    std::process::exit(2);
                }
            },
            None => Value::Null,
        };
        let query: RealDataRefreshAuditQuery = if query_document.is_null() {
            RealDataRefreshAuditQuery::default()
        } else {
            match serde_json::from_value(query_document) {
                Ok(query) => query,
                Err(error) => {
                    emit_error(&format!(
                        "invalid real-data refresh-audit query JSON: {error}"
                    ));
                    std::process::exit(2);
                }
            }
        };
        match agent.real_data_refresh_audit(&request, &before, &after, &query) {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("real-data refresh-audit report is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some((before_path, after_path)) = public_literature_refresh_audit_paths.as_ref() {
        if real_data_path.is_some()
            || public_literature_path.is_some()
            || query_data_path.is_some()
            || public_query_data_path.is_some()
            || evidence_graph_path.is_some()
            || real_data_coverage_path.is_some()
            || real_data_freshness_path.is_some()
            || real_data_diff_paths.is_some()
            || real_data_refresh_audit_paths.is_some()
            || real_data_review_queue_path.is_some()
            || real_data_review_disposition_path.is_some()
            || real_data_evidence_packet_path.is_some()
            || real_data_reasoning_context_path.is_some()
            || real_data_draft_audit_path.is_some()
            || public_literature_evidence_packet_path.is_some()
            || public_literature_reasoning_context_path.is_some()
            || public_literature_draft_audit_path.is_some()
            || public_literature_matrix_path.is_some()
            || public_literature_freshness_path.is_some()
            || session_start
            || session_advance.is_some()
            || session_finish.is_some()
            || session_run
            || mission
            || research_plan
            || research_brief
            || audit_evidence
        {
            emit_error(
                "--public-literature-refresh-audit accepts only two bundle paths and a refresh query on stdin",
            );
            std::process::exit(2);
        }
        let before = match read_public_literature(before_path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let after = match read_public_literature(after_path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let query: PublicLiteratureRefreshAuditQuery = match serde_json::from_value(document) {
            Ok(query) => query,
            Err(error) => {
                emit_error(&format!(
                    "invalid public-literature refresh-audit query JSON: {error}"
                ));
                std::process::exit(2);
            }
        };
        match agent.public_literature_refresh_audit(&before, &after, &query) {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("public-literature refresh-audit report is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some((real_path, public_path)) = literature_link_audit_paths.as_ref() {
        if real_data_path.is_some()
            || public_literature_path.is_some()
            || query_data_path.is_some()
            || public_query_data_path.is_some()
            || evidence_graph_path.is_some()
            || real_data_coverage_path.is_some()
            || real_data_freshness_path.is_some()
            || real_data_diff_paths.is_some()
            || real_data_refresh_audit_paths.is_some()
            || public_literature_refresh_audit_paths.is_some()
            || real_data_review_queue_path.is_some()
            || real_data_review_disposition_path.is_some()
            || real_data_evidence_packet_path.is_some()
            || real_data_reasoning_context_path.is_some()
            || real_data_draft_audit_path.is_some()
            || public_literature_evidence_packet_path.is_some()
            || public_literature_reasoning_context_path.is_some()
            || public_literature_draft_audit_path.is_some()
            || public_literature_matrix_path.is_some()
            || public_literature_freshness_path.is_some()
            || session_start
            || session_advance.is_some()
            || session_finish.is_some()
            || session_run
            || mission
            || research_plan
            || research_brief
            || audit_evidence
        {
            emit_error(
                "--literature-link-audit accepts only a real glioma path, a public-literature path, and a link query on stdin",
            );
            std::process::exit(2);
        }
        let real_data = match read_real_data(real_path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let public_literature = match read_public_literature(public_path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let query: LiteratureLinkAuditQuery = match serde_json::from_value(document) {
            Ok(query) => query,
            Err(error) => {
                emit_error(&format!(
                    "invalid literature link-audit query JSON: {error}"
                ));
                std::process::exit(2);
            }
        };
        match agent.literature_link_audit(&real_data, &public_literature, &query) {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("literature link-audit report is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some(path) = public_literature_integrity_audit_path.as_deref() {
        if real_data_path.is_some()
            || public_literature_path.is_some()
            || query_data_path.is_some()
            || public_query_data_path.is_some()
            || evidence_graph_path.is_some()
            || real_data_coverage_path.is_some()
            || real_data_freshness_path.is_some()
            || real_data_diff_paths.is_some()
            || real_data_refresh_audit_paths.is_some()
            || public_literature_refresh_audit_paths.is_some()
            || literature_link_audit_paths.is_some()
            || real_data_review_queue_path.is_some()
            || real_data_review_disposition_path.is_some()
            || real_data_evidence_packet_path.is_some()
            || real_data_reasoning_context_path.is_some()
            || real_data_draft_audit_path.is_some()
            || public_literature_evidence_packet_path.is_some()
            || public_literature_reasoning_context_path.is_some()
            || public_literature_draft_audit_path.is_some()
            || public_literature_matrix_path.is_some()
            || public_literature_freshness_path.is_some()
            || session_start
            || session_advance.is_some()
            || session_finish.is_some()
            || session_run
            || mission
            || research_plan
            || research_brief
            || audit_evidence
        {
            emit_error(
                "--public-literature-integrity-audit accepts only its bundle path and an integrity query on stdin",
            );
            std::process::exit(2);
        }
        let public_literature = match read_public_literature(path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let query: PublicLiteratureIntegrityAuditQuery = match serde_json::from_value(document) {
            Ok(query) => query,
            Err(error) => {
                emit_error(&format!(
                    "invalid public-literature integrity-audit query JSON: {error}"
                ));
                std::process::exit(2);
            }
        };
        match agent.public_literature_integrity_audit(&public_literature, &query) {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("public-literature integrity-audit report is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some(path) = public_literature_review_queue_path.as_deref() {
        if real_data_path.is_some()
            || public_literature_path.is_some()
            || query_data_path.is_some()
            || public_query_data_path.is_some()
            || evidence_graph_path.is_some()
            || real_data_coverage_path.is_some()
            || real_data_freshness_path.is_some()
            || real_data_diff_paths.is_some()
            || real_data_refresh_audit_paths.is_some()
            || public_literature_refresh_audit_paths.is_some()
            || literature_link_audit_paths.is_some()
            || real_data_review_queue_path.is_some()
            || real_data_review_disposition_path.is_some()
            || real_data_evidence_packet_path.is_some()
            || real_data_reasoning_context_path.is_some()
            || real_data_draft_audit_path.is_some()
            || public_literature_evidence_packet_path.is_some()
            || public_literature_reasoning_context_path.is_some()
            || public_literature_draft_audit_path.is_some()
            || public_literature_matrix_path.is_some()
            || public_literature_freshness_path.is_some()
            || public_literature_integrity_audit_path.is_some()
            || session_start
            || session_advance.is_some()
            || session_finish.is_some()
            || session_run
            || mission
            || research_plan
            || research_brief
            || audit_evidence
        {
            emit_error(
                "--public-literature-review-queue accepts only its bundle path and a review-queue query on stdin",
            );
            std::process::exit(2);
        }
        let data = match read_public_literature(path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let query: PublicLiteratureReviewQueueQuery = match serde_json::from_value(document) {
            Ok(query) => query,
            Err(error) => {
                emit_error(&format!(
                    "invalid public-literature review-queue query JSON: {error}"
                ));
                std::process::exit(2);
            }
        };
        match agent.public_literature_review_queue(&data, &query) {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("public-literature review queue is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some(path) = public_literature_workbench_path.as_deref() {
        if real_data_path.is_some()
            || public_literature_path.is_some()
            || query_data_path.is_some()
            || public_query_data_path.is_some()
            || evidence_graph_path.is_some()
            || real_data_coverage_path.is_some()
            || real_data_freshness_path.is_some()
            || real_data_diff_paths.is_some()
            || real_data_refresh_audit_paths.is_some()
            || public_literature_refresh_audit_paths.is_some()
            || literature_link_audit_paths.is_some()
            || real_data_review_queue_path.is_some()
            || real_data_review_disposition_path.is_some()
            || real_data_evidence_packet_path.is_some()
            || real_data_reasoning_context_path.is_some()
            || real_data_draft_audit_path.is_some()
            || public_literature_evidence_packet_path.is_some()
            || public_literature_reasoning_context_path.is_some()
            || public_literature_draft_audit_path.is_some()
            || public_literature_matrix_path.is_some()
            || public_literature_freshness_path.is_some()
            || public_literature_integrity_audit_path.is_some()
            || public_literature_review_queue_path.is_some()
            || session_start
            || session_advance.is_some()
            || session_finish.is_some()
            || session_run
            || mission
            || research_plan
            || research_brief
            || audit_evidence
        {
            emit_error(
                "--public-literature-workbench accepts only its bundle path and a workbench query on stdin",
            );
            std::process::exit(2);
        }
        let data = match read_public_literature(path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let query: PublicLiteratureWorkbenchQuery = match serde_json::from_value(document) {
            Ok(query) => query,
            Err(error) => {
                emit_error(&format!(
                    "invalid public-literature workbench query JSON: {error}"
                ));
                std::process::exit(2);
            }
        };
        match agent.public_literature_workbench(&data, &query) {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("public-literature workbench is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some(path) = public_literature_portfolio_path.as_deref() {
        if real_data_path.is_some()
            || public_literature_path.is_some()
            || query_data_path.is_some()
            || public_query_data_path.is_some()
            || evidence_graph_path.is_some()
            || real_data_coverage_path.is_some()
            || real_data_freshness_path.is_some()
            || real_data_diff_paths.is_some()
            || real_data_refresh_audit_paths.is_some()
            || public_literature_refresh_audit_paths.is_some()
            || literature_link_audit_paths.is_some()
            || real_data_review_queue_path.is_some()
            || real_data_review_disposition_path.is_some()
            || real_data_evidence_packet_path.is_some()
            || real_data_reasoning_context_path.is_some()
            || real_data_draft_audit_path.is_some()
            || public_literature_evidence_packet_path.is_some()
            || public_literature_reasoning_context_path.is_some()
            || public_literature_draft_audit_path.is_some()
            || public_literature_matrix_path.is_some()
            || public_literature_freshness_path.is_some()
            || public_literature_integrity_audit_path.is_some()
            || public_literature_review_queue_path.is_some()
            || public_literature_workbench_path.is_some()
            || session_start
            || session_advance.is_some()
            || session_finish.is_some()
            || session_run
            || mission
            || research_plan
            || research_brief
            || audit_evidence
        {
            emit_error(
                "--public-literature-portfolio accepts only its bundle path and a portfolio query on stdin",
            );
            std::process::exit(2);
        }
        let data = match read_public_literature(path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let query: PublicLiteraturePortfolioQuery = match serde_json::from_value(document) {
            Ok(query) => query,
            Err(error) => {
                emit_error(&format!(
                    "invalid public-literature portfolio query JSON: {error}"
                ));
                std::process::exit(2);
            }
        };
        match agent.public_literature_portfolio(&data, &query) {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("public-literature portfolio is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some(path) = real_data_coverage_path.as_deref() {
        if real_data_path.is_some()
            || public_literature_path.is_some()
            || query_data_path.is_some()
            || public_query_data_path.is_some()
            || evidence_graph_path.is_some()
            || real_data_review_disposition_path.is_some()
            || real_data_evidence_packet_path.is_some()
            || real_data_draft_audit_path.is_some()
            || session_start
            || session_advance.is_some()
            || session_finish.is_some()
            || session_run
            || mission
            || research_plan
            || audit_evidence
        {
            emit_error(
                "--real-data-coverage accepts only its bundle path and a RealDataCoverageQuery on stdin",
            );
            std::process::exit(2);
        }
        let data = match read_real_data(path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let query: RealDataCoverageQuery = match serde_json::from_value(document) {
            Ok(query) => query,
            Err(error) => {
                emit_error(&format!("invalid real-data coverage query JSON: {error}"));
                std::process::exit(2);
            }
        };
        match agent.real_data_coverage(&data, &query) {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("real-data coverage report is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some(path) = real_data_reconciliation_path.as_deref() {
        if real_data_path.is_some()
            || public_literature_path.is_some()
            || query_data_path.is_some()
            || public_query_data_path.is_some()
            || evidence_graph_path.is_some()
            || real_data_coverage_path.is_some()
            || real_data_freshness_path.is_some()
            || real_data_diff_paths.is_some()
            || real_data_refresh_audit_paths.is_some()
            || real_data_review_queue_path.is_some()
            || real_data_review_disposition_path.is_some()
            || real_data_evidence_packet_path.is_some()
            || real_data_reasoning_context_path.is_some()
            || real_data_draft_audit_path.is_some()
            || real_data_trial_landscape_path.is_some()
            || real_data_molecular_coverage_path.is_some()
            || public_literature_evidence_packet_path.is_some()
            || public_literature_reasoning_context_path.is_some()
            || public_literature_draft_audit_path.is_some()
            || public_literature_matrix_path.is_some()
            || public_literature_freshness_path.is_some()
            || public_literature_refresh_audit_paths.is_some()
            || literature_link_audit_paths.is_some()
            || public_literature_integrity_audit_path.is_some()
            || public_literature_review_queue_path.is_some()
            || public_literature_workbench_path.is_some()
            || public_literature_portfolio_path.is_some()
            || session_start
            || session_advance.is_some()
            || session_finish.is_some()
            || session_run
            || mission
            || research_plan
            || audit_evidence
        {
            emit_error(
                "--real-data-reconciliation accepts only its bundle path and a RealDataReconciliationQuery on stdin",
            );
            std::process::exit(2);
        }
        let data = match read_real_data(path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let query: RealDataReconciliationQuery = match serde_json::from_value(document) {
            Ok(query) => query,
            Err(error) => {
                emit_error(&format!(
                    "invalid real-data reconciliation query JSON: {error}"
                ));
                std::process::exit(2);
            }
        };
        match agent.real_data_reconciliation(&data, &query) {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("real-data reconciliation report is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some(path) = real_data_freshness_path.as_deref() {
        if real_data_path.is_some()
            || public_literature_path.is_some()
            || query_data_path.is_some()
            || public_query_data_path.is_some()
            || evidence_graph_path.is_some()
            || real_data_coverage_path.is_some()
            || real_data_diff_paths.is_some()
            || real_data_review_queue_path.is_some()
            || real_data_review_disposition_path.is_some()
            || real_data_evidence_packet_path.is_some()
            || real_data_reasoning_context_path.is_some()
            || real_data_draft_audit_path.is_some()
            || public_literature_evidence_packet_path.is_some()
            || public_literature_reasoning_context_path.is_some()
            || public_literature_draft_audit_path.is_some()
            || public_literature_matrix_path.is_some()
            || session_start
            || session_advance.is_some()
            || session_finish.is_some()
            || session_run
            || mission
            || research_plan
            || audit_evidence
        {
            emit_error(
                "--real-data-freshness accepts only a real-glioma bundle path and a RealDataFreshnessQuery on stdin",
            );
            std::process::exit(2);
        }
        let data = match read_real_data(path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let query: RealDataFreshnessQuery = match serde_json::from_value(document) {
            Ok(query) => query,
            Err(error) => {
                emit_error(&format!("invalid real-data freshness query JSON: {error}"));
                std::process::exit(2);
            }
        };
        match agent.real_data_freshness(&data, &query) {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("real-data freshness report is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some(path) = real_data_review_queue_path.as_deref() {
        if real_data_path.is_some()
            || public_literature_path.is_some()
            || query_data_path.is_some()
            || public_query_data_path.is_some()
            || evidence_graph_path.is_some()
            || real_data_coverage_path.is_some()
            || real_data_diff_paths.is_some()
            || real_data_review_disposition_path.is_some()
            || real_data_evidence_packet_path.is_some()
            || real_data_draft_audit_path.is_some()
            || session_start
            || session_advance.is_some()
            || session_finish.is_some()
            || session_run
            || mission
            || research_plan
            || audit_evidence
        {
            emit_error(
                "--real-data-review-queue accepts only its bundle path and a RealDataReviewQueueQuery on stdin",
            );
            std::process::exit(2);
        }
        let data = match read_real_data(path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let query: RealDataReviewQueueQuery = match serde_json::from_value(document) {
            Ok(query) => query,
            Err(error) => {
                emit_error(&format!(
                    "invalid real-data review-queue query JSON: {error}"
                ));
                std::process::exit(2);
            }
        };
        match agent.real_data_review_queue(&data, &query) {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("real-data review queue is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some(path) = real_data_review_disposition_path.as_deref() {
        if real_data_path.is_some()
            || public_literature_path.is_some()
            || query_data_path.is_some()
            || public_query_data_path.is_some()
            || evidence_graph_path.is_some()
            || real_data_coverage_path.is_some()
            || real_data_diff_paths.is_some()
            || real_data_review_queue_path.is_some()
            || real_data_evidence_packet_path.is_some()
            || real_data_draft_audit_path.is_some()
            || session_start
            || session_advance.is_some()
            || session_finish.is_some()
            || session_run
            || mission
            || research_plan
            || audit_evidence
        {
            emit_error(
                "--real-data-review-disposition accepts only a queue report path and a JSON decision array on stdin",
            );
            std::process::exit(2);
        }
        let queue_text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) => {
                emit_error(&format!(
                    "could not read real-data review queue {path:?}: {error}"
                ));
                std::process::exit(2);
            }
        };
        let queue = match serde_json::from_str::<bioprism_neurosurgery::RealDataReviewQueueReport>(
            &queue_text,
        ) {
            Ok(queue) => queue,
            Err(error) => {
                emit_error(&format!("invalid real-data review queue JSON: {error}"));
                std::process::exit(2);
            }
        };
        let decisions = match serde_json::from_value::<Vec<RealDataReviewDecision>>(document) {
            Ok(decisions) => decisions,
            Err(error) => {
                emit_error(&format!("invalid real-data review decision JSON: {error}"));
                std::process::exit(2);
            }
        };
        if decisions.len() > MAX_REAL_DATA_REVIEW_DISPOSITIONS {
            emit_error(&format!(
                "real-data review decisions must contain at most {MAX_REAL_DATA_REVIEW_DISPOSITIONS} items"
            ));
            std::process::exit(2);
        }
        match agent.real_data_review_disposition(&queue, &decisions) {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("real-data review disposition is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some(path) = real_data_evidence_packet_path.as_deref() {
        if real_data_path.is_some()
            || public_literature_path.is_some()
            || query_data_path.is_some()
            || public_query_data_path.is_some()
            || evidence_graph_path.is_some()
            || real_data_coverage_path.is_some()
            || real_data_diff_paths.is_some()
            || real_data_review_queue_path.is_some()
            || real_data_review_disposition_path.is_some()
            || real_data_draft_audit_path.is_some()
            || session_start
            || session_advance.is_some()
            || session_finish.is_some()
            || session_run
            || mission
            || research_plan
            || audit_evidence
        {
            emit_error(
                "--real-data-evidence-packet accepts only a bundle path and a RealDataEvidencePacketQuery on stdin",
            );
            std::process::exit(2);
        }
        let data = match read_real_data(path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let query: RealDataEvidencePacketQuery = match serde_json::from_value(document) {
            Ok(query) => query,
            Err(error) => {
                emit_error(&format!(
                    "invalid real-data evidence-packet query JSON: {error}"
                ));
                std::process::exit(2);
            }
        };
        match agent.real_data_evidence_packet(&data, &query) {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("real-data evidence packet is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some(path) = real_data_autonomous_workflow_path.as_deref() {
        if real_data_path.is_some()
            || public_literature_path.is_some()
            || query_data_path.is_some()
            || public_query_data_path.is_some()
            || evidence_graph_path.is_some()
            || real_data_coverage_path.is_some()
            || real_data_diff_paths.is_some()
            || real_data_refresh_audit_paths.is_some()
            || real_data_review_queue_path.is_some()
            || real_data_review_disposition_path.is_some()
            || real_data_evidence_packet_path.is_some()
            || real_data_reasoning_context_path.is_some()
            || real_data_draft_audit_path.is_some()
            || session_start
            || session_advance.is_some()
            || session_finish.is_some()
            || session_run
            || mission
            || research_plan
            || audit_evidence
        {
            emit_error(
                "--real-data-autonomous-workflow accepts only a bundle path and a RealDataAutonomousWorkflowQuery on stdin",
            );
            std::process::exit(2);
        }
        let data = match read_real_data(path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let query: RealDataAutonomousWorkflowQuery = match serde_json::from_value(document) {
            Ok(query) => query,
            Err(error) => {
                emit_error(&format!(
                    "invalid real-data autonomous-workflow query JSON: {error}"
                ));
                std::process::exit(2);
            }
        };
        match agent.real_data_autonomous_workflow(&data, &query) {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("real-data autonomous workflow is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some(path) = real_data_draft_audit_path.as_deref() {
        if real_data_path.is_some()
            || public_literature_path.is_some()
            || query_data_path.is_some()
            || public_query_data_path.is_some()
            || evidence_graph_path.is_some()
            || real_data_coverage_path.is_some()
            || real_data_diff_paths.is_some()
            || real_data_review_queue_path.is_some()
            || real_data_review_disposition_path.is_some()
            || real_data_evidence_packet_path.is_some()
            || session_start
            || session_advance.is_some()
            || session_finish.is_some()
            || session_run
            || mission
            || research_plan
            || audit_evidence
        {
            emit_error(
                "--real-data-draft-audit accepts only a bundle path and a draft JSON object on stdin",
            );
            std::process::exit(2);
        }
        let data = match read_real_data(path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let request: RealDataDraftAuditRequest = match serde_json::from_value(document) {
            Ok(request) => request,
            Err(error) => {
                emit_error(&format!("invalid real-data draft audit JSON: {error}"));
                std::process::exit(2);
            }
        };
        match agent.real_data_draft_audit(&data, &request) {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("real-data draft audit report is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some(path) = real_data_reasoning_context_path.as_deref() {
        if real_data_path.is_some()
            || public_literature_path.is_some()
            || query_data_path.is_some()
            || public_query_data_path.is_some()
            || evidence_graph_path.is_some()
            || real_data_coverage_path.is_some()
            || real_data_diff_paths.is_some()
            || real_data_review_queue_path.is_some()
            || real_data_review_disposition_path.is_some()
            || real_data_evidence_packet_path.is_some()
            || real_data_draft_audit_path.is_some()
            || public_literature_evidence_packet_path.is_some()
            || public_literature_draft_audit_path.is_some()
            || public_literature_matrix_path.is_some()
            || session_start
            || session_advance.is_some()
            || session_finish.is_some()
            || session_run
            || mission
            || research_plan
            || audit_evidence
        {
            emit_error(
                "--real-data-reasoning-context accepts only a bundle path and a RealDataReasoningContextQuery on stdin",
            );
            std::process::exit(2);
        }
        let data = match read_real_data(path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let query: RealDataReasoningContextQuery = match serde_json::from_value(document) {
            Ok(query) => query,
            Err(error) => {
                emit_error(&format!(
                    "invalid real-data reasoning-context query JSON: {error}"
                ));
                std::process::exit(2);
            }
        };
        match agent.real_data_reasoning_context(&data, &query) {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("real-data reasoning context is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some(path) = public_literature_evidence_packet_path.as_deref() {
        if real_data_path.is_some()
            || public_literature_path.is_some()
            || query_data_path.is_some()
            || public_query_data_path.is_some()
            || evidence_graph_path.is_some()
            || real_data_coverage_path.is_some()
            || real_data_diff_paths.is_some()
            || real_data_review_queue_path.is_some()
            || real_data_review_disposition_path.is_some()
            || real_data_evidence_packet_path.is_some()
            || real_data_draft_audit_path.is_some()
            || session_start
            || session_advance.is_some()
            || session_finish.is_some()
            || session_run
            || mission
            || research_plan
            || audit_evidence
        {
            emit_error(
                "--public-literature-evidence-packet accepts only a public-literature bundle path and a PublicLiteratureEvidencePacketQuery on stdin",
            );
            std::process::exit(2);
        }
        let data = match read_public_literature(path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let query: PublicLiteratureEvidencePacketQuery = match serde_json::from_value(document) {
            Ok(query) => query,
            Err(error) => {
                emit_error(&format!(
                    "invalid public-literature evidence-packet query JSON: {error}"
                ));
                std::process::exit(2);
            }
        };
        match agent.public_literature_evidence_packet(&data, &query) {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("public-literature evidence packet is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some(path) = public_literature_freshness_path.as_deref() {
        if real_data_path.is_some()
            || public_literature_path.is_some()
            || query_data_path.is_some()
            || public_query_data_path.is_some()
            || evidence_graph_path.is_some()
            || real_data_coverage_path.is_some()
            || real_data_freshness_path.is_some()
            || real_data_diff_paths.is_some()
            || real_data_review_queue_path.is_some()
            || real_data_review_disposition_path.is_some()
            || real_data_evidence_packet_path.is_some()
            || real_data_reasoning_context_path.is_some()
            || real_data_draft_audit_path.is_some()
            || public_literature_evidence_packet_path.is_some()
            || public_literature_reasoning_context_path.is_some()
            || public_literature_draft_audit_path.is_some()
            || public_literature_matrix_path.is_some()
            || session_start
            || session_advance.is_some()
            || session_finish.is_some()
            || session_run
            || mission
            || research_plan
            || audit_evidence
        {
            emit_error(
                "--public-literature-freshness accepts only a public-literature bundle path and a RealDataFreshnessQuery on stdin",
            );
            std::process::exit(2);
        }
        let data = match read_public_literature(path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let query: RealDataFreshnessQuery = match serde_json::from_value(document) {
            Ok(query) => query,
            Err(error) => {
                emit_error(&format!(
                    "invalid public-literature freshness query JSON: {error}"
                ));
                std::process::exit(2);
            }
        };
        match agent.public_literature_freshness(&data, &query) {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("public-literature freshness report is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some(path) = public_literature_reasoning_context_path.as_deref() {
        if real_data_path.is_some()
            || public_literature_path.is_some()
            || query_data_path.is_some()
            || public_query_data_path.is_some()
            || evidence_graph_path.is_some()
            || real_data_coverage_path.is_some()
            || real_data_diff_paths.is_some()
            || real_data_review_queue_path.is_some()
            || real_data_review_disposition_path.is_some()
            || real_data_evidence_packet_path.is_some()
            || real_data_reasoning_context_path.is_some()
            || real_data_draft_audit_path.is_some()
            || public_literature_evidence_packet_path.is_some()
            || public_literature_draft_audit_path.is_some()
            || public_literature_matrix_path.is_some()
            || session_start
            || session_advance.is_some()
            || session_finish.is_some()
            || session_run
            || mission
            || research_plan
            || audit_evidence
        {
            emit_error(
                "--public-literature-reasoning-context accepts only a public-literature bundle path and a PublicLiteratureReasoningContextQuery on stdin",
            );
            std::process::exit(2);
        }
        let data = match read_public_literature(path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let query: PublicLiteratureReasoningContextQuery = match serde_json::from_value(document) {
            Ok(query) => query,
            Err(error) => {
                emit_error(&format!(
                    "invalid public-literature reasoning-context query JSON: {error}"
                ));
                std::process::exit(2);
            }
        };
        match agent.public_literature_reasoning_context(&data, &query) {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("public-literature reasoning context is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some(path) = public_literature_draft_audit_path.as_deref() {
        if real_data_path.is_some()
            || public_literature_path.is_some()
            || query_data_path.is_some()
            || public_query_data_path.is_some()
            || evidence_graph_path.is_some()
            || real_data_coverage_path.is_some()
            || real_data_diff_paths.is_some()
            || real_data_review_queue_path.is_some()
            || real_data_review_disposition_path.is_some()
            || real_data_evidence_packet_path.is_some()
            || real_data_draft_audit_path.is_some()
            || public_literature_evidence_packet_path.is_some()
            || session_start
            || session_advance.is_some()
            || session_finish.is_some()
            || session_run
            || mission
            || research_plan
            || audit_evidence
        {
            emit_error(
                "--public-literature-draft-audit accepts only a public-literature bundle path and a PublicLiteratureDraftAuditRequest on stdin",
            );
            std::process::exit(2);
        }
        let data = match read_public_literature(path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let request: PublicLiteratureDraftAuditRequest = match serde_json::from_value(document) {
            Ok(request) => request,
            Err(error) => {
                emit_error(&format!(
                    "invalid public-literature draft audit JSON: {error}"
                ));
                std::process::exit(2);
            }
        };
        match agent.public_literature_draft_audit(&data, &request) {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("public-literature draft audit report is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some(path) = public_literature_matrix_path.as_deref() {
        if real_data_path.is_some()
            || public_literature_path.is_some()
            || query_data_path.is_some()
            || public_query_data_path.is_some()
            || evidence_graph_path.is_some()
            || real_data_coverage_path.is_some()
            || real_data_diff_paths.is_some()
            || real_data_review_queue_path.is_some()
            || real_data_review_disposition_path.is_some()
            || real_data_evidence_packet_path.is_some()
            || real_data_draft_audit_path.is_some()
            || public_literature_evidence_packet_path.is_some()
            || public_literature_draft_audit_path.is_some()
            || session_start
            || session_advance.is_some()
            || session_finish.is_some()
            || session_run
            || mission
            || research_plan
            || audit_evidence
        {
            emit_error(
                "--public-literature-matrix accepts only a public-literature bundle path and a PublicLiteratureMatrixQuery on stdin",
            );
            std::process::exit(2);
        }
        let data = match read_public_literature(path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let query: PublicLiteratureMatrixQuery = match serde_json::from_value(document) {
            Ok(query) => query,
            Err(error) => {
                emit_error(&format!("invalid public-literature matrix JSON: {error}"));
                std::process::exit(2);
            }
        };
        match agent.public_literature_matrix(&data, &query) {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("public-literature matrix report is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some(path) = evidence_graph_path.as_deref() {
        if real_data_path.is_some()
            || public_literature_path.is_some()
            || query_data_path.is_some()
            || public_query_data_path.is_some()
            || session_start
            || session_advance.is_some()
            || session_finish.is_some()
            || session_run
            || mission
            || research_plan
            || audit_evidence
        {
            emit_error(
                "--evidence-graph accepts only its bundle path and an EvidenceGraphQuery on stdin",
            );
            std::process::exit(2);
        }
        let data = match read_real_data(path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let query: EvidenceGraphQuery = match serde_json::from_value(document) {
            Ok(query) => query,
            Err(error) => {
                emit_error(&format!("invalid evidence-graph query JSON: {error}"));
                std::process::exit(2);
            }
        };
        match agent.evidence_graph(&data, &query) {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report).expect("evidence graph is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }
    let real_data = match real_data_path.as_deref() {
        Some(path) => match read_real_data(path) {
            Ok(data) => Some(data),
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        },
        None => None,
    };
    let public_literature = match public_literature_path.as_deref() {
        Some(path) => match read_public_literature(path) {
            Ok(data) => Some(data),
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        },
        None => None,
    };

    let intake_case_asset_manifest = if intake_mission || intake_portfolio {
        match case_asset_manifest_path.as_deref() {
            Some(path) => match read_case_asset_manifest(path) {
                Ok(manifest) => Some(manifest),
                Err(error) => {
                    emit_error(&error);
                    std::process::exit(2);
                }
            },
            None => None,
        }
    } else {
        None
    };
    let intake_case_asset_query = if intake_mission || intake_portfolio {
        match case_asset_manifest_query_path.as_deref() {
            Some(path) => match read_query_value(path) {
                Ok(value) => match serde_json::from_value::<CaseAssetManifestQuery>(value) {
                    Ok(query) => Some(query),
                    Err(error) => {
                        emit_error(&format!("invalid case-asset-manifest query JSON: {error}"));
                        std::process::exit(2);
                    }
                },
                Err(error) => {
                    emit_error(&error);
                    std::process::exit(2);
                }
            },
            None => None,
        }
    } else {
        None
    };
    let intake_case_asset_disposition = if intake_mission || intake_portfolio {
        match intake_case_asset_review_disposition_path.as_deref() {
            Some(path) => match std::fs::read_to_string(path) {
                Ok(text) => match serde_json::from_str::<CaseAssetReviewDispositionReport>(&text) {
                    Ok(report) => Some(report),
                    Err(error) => {
                        emit_error(&format!(
                            "invalid intake case-asset disposition report JSON: {error}"
                        ));
                        std::process::exit(2);
                    }
                },
                Err(error) => {
                    emit_error(&format!(
                        "could not read intake case-asset disposition report {path:?}: {error}"
                    ));
                    std::process::exit(2);
                }
            },
            None => None,
        }
    } else {
        None
    };
    let intake_case_dicom_import = if intake_mission {
        match intake_case_dicom_path.as_deref() {
            Some(path) => match read_dicom_case_import(path) {
                Ok(import) => Some(import),
                Err(error) => {
                    emit_error(&error);
                    std::process::exit(2);
                }
            },
            None => None,
        }
    } else {
        None
    };
    let intake_case_fhir_import = if intake_mission {
        match intake_case_fhir_path.as_deref() {
            Some(path) => match read_fhir_case_import(path) {
                Ok(import) => Some(import),
                Err(error) => {
                    emit_error(&error);
                    std::process::exit(2);
                }
            },
            None => None,
        }
    } else {
        None
    };
    let intake_freshness = if intake_mission || intake_portfolio {
        if intake_freshness_path.is_some()
            && document
                .get("freshness")
                .is_some_and(|value| !value.is_null())
        {
            emit_error(
                "provide intake freshness either inline in stdin or with --intake-freshness, not both",
            );
            std::process::exit(2);
        }
        match intake_freshness_path.as_deref() {
            Some(path) => match read_query_value(path) {
                Ok(value) => match serde_json::from_value::<RealDataFreshnessQuery>(value) {
                    Ok(query) => Some(query),
                    Err(error) => {
                        emit_error(&format!("invalid intake freshness query JSON: {error}"));
                        std::process::exit(2);
                    }
                },
                Err(error) => {
                    emit_error(&error);
                    std::process::exit(2);
                }
            },
            None => match document.get("freshness").filter(|value| !value.is_null()) {
                Some(value) => {
                    match serde_json::from_value::<RealDataFreshnessQuery>(value.clone()) {
                        Ok(query) => Some(query),
                        Err(error) => {
                            emit_error(&format!(
                                "invalid inline intake freshness query JSON: {error}"
                            ));
                            std::process::exit(2);
                        }
                    }
                }
                None => None,
            },
        }
    } else {
        None
    };

    if intake_mission || intake_portfolio {
        let result = if intake_mission {
            let query: NeurosurgicalIntakeQuery = match serde_json::from_value(document.clone()) {
                Ok(query) => query,
                Err(error) => {
                    emit_error(&format!(
                        "invalid neurosurgical intake mission JSON: {error}"
                    ));
                    std::process::exit(2);
                }
            };
            let result = if intake_case_dicom_import.is_some() || intake_case_fhir_import.is_some()
            {
                agent.run_intake_mission_with_case_imports(
                    &query,
                    real_data.as_ref(),
                    public_literature.as_ref(),
                    intake_case_dicom_import.as_ref(),
                    intake_case_fhir_import.as_ref(),
                    intake_freshness.as_ref(),
                    max_session_steps,
                )
            } else {
                // The compatibility method is intentionally not used here: CLI intake must
                // preserve the caller-owned freshness posture in the mission envelope.
                agent.run_intake_mission_with_case_assets_and_dispositions(
                    &query,
                    real_data.as_ref(),
                    public_literature.as_ref(),
                    intake_case_asset_manifest.as_ref(),
                    intake_case_asset_query.as_ref(),
                    intake_freshness.as_ref(),
                    intake_case_asset_disposition.as_ref(),
                    max_session_steps,
                )
            };
            result.map(|value| serde_json::to_value(value).expect("intake mission is serialisable"))
        } else {
            let mut query: NeurosurgicalIntakePortfolioQuery =
                match serde_json::from_value(document.clone()) {
                    Ok(query) => query,
                    Err(error) => {
                        emit_error(&format!(
                            "invalid neurosurgical intake portfolio JSON: {error}"
                        ));
                        std::process::exit(2);
                    }
                };
            if max_session_steps_arg.is_some() {
                query.max_session_steps = max_session_steps;
            }
            agent
                .run_intake_portfolio_with_case_assets_and_freshness_and_dispositions(
                    &query,
                    real_data.as_ref(),
                    public_literature.as_ref(),
                    intake_case_asset_manifest.as_ref(),
                    intake_case_asset_query.as_ref(),
                    intake_freshness.as_ref(),
                    intake_case_asset_disposition.as_ref(),
                )
                .map(|value| serde_json::to_value(value).expect("intake portfolio is serialisable"))
        };
        match result {
            Ok(value) => println!("{}", serde_json::to_string_pretty(&value).expect("JSON")),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some(path) = query_data_path {
        if real_data_path.is_some()
            || public_literature_path.is_some()
            || session_start
            || session_advance.is_some()
            || session_finish.is_some()
            || session_run
            || mission
            || research_plan
            || real_data_molecular_coverage_path.is_some()
        {
            emit_error("--query-real-glioma cannot be combined with another run mode");
            std::process::exit(2);
        }
        let data = match read_real_data(&path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let query: RealDataQuery = match serde_json::from_value(document) {
            Ok(query) => query,
            Err(error) => {
                emit_error(&format!("invalid real-data query JSON: {error}"));
                std::process::exit(2);
            }
        };
        match data.query(&query) {
            Ok(result) => println!(
                "{}",
                serde_json::to_string_pretty(&result).expect("query result is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some(path) = real_data_trial_landscape_path {
        let data = match read_real_data(&path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let query: RealDataTrialLandscapeQuery = match serde_json::from_value(document) {
            Ok(query) => query,
            Err(error) => {
                emit_error(&format!(
                    "invalid real-data trial-landscape query JSON: {error}"
                ));
                std::process::exit(2);
            }
        };
        match data.trial_landscape(&query) {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("trial landscape report is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some(path) = real_data_molecular_coverage_path {
        let data = match read_real_data(&path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let query: RealDataMolecularCoverageQuery = match serde_json::from_value(document) {
            Ok(query) => query,
            Err(error) => {
                emit_error(&format!(
                    "invalid real-data molecular-coverage query JSON: {error}"
                ));
                std::process::exit(2);
            }
        };
        match data.molecular_coverage(&query) {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("molecular coverage report is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some(path) = real_data_cohort_landscape_path {
        let data = match read_real_data(&path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let query: RealDataCohortLandscapeQuery = match serde_json::from_value(document) {
            Ok(query) => query,
            Err(error) => {
                emit_error(&format!(
                    "invalid real-data cohort-landscape query JSON: {error}"
                ));
                std::process::exit(2);
            }
        };
        match data.cohort_landscape(&query) {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("cohort landscape report is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if let Some(path) = public_query_data_path {
        if real_data_path.is_some()
            || public_literature_path.is_some()
            || session_start
            || session_advance.is_some()
            || session_finish.is_some()
            || session_run
            || mission
            || research_plan
        {
            emit_error("--query-public-literature cannot be combined with another run mode");
            std::process::exit(2);
        }
        let data = match read_public_literature(&path) {
            Ok(data) => data,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let query: PublicLiteratureQuery = match serde_json::from_value(document) {
            Ok(query) => query,
            Err(error) => {
                emit_error(&format!("invalid public-literature query JSON: {error}"));
                std::process::exit(2);
            }
        };
        match data.query(&query) {
            Ok(result) => println!(
                "{}",
                serde_json::to_string_pretty(&result).expect("query result is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if research_plan {
        let request: CaseRequest = match serde_json::from_value(document) {
            Ok(request) => request,
            Err(error) => {
                emit_error(&format!("invalid neurosurgical request JSON: {error}"));
                std::process::exit(2);
            }
        };
        if autonomous_acquisition {
            let query = match autonomous_acquisition_query_path.as_deref() {
                Some(path) => match read_query_value(path) {
                    Ok(value) => match serde_json::from_value::<EvidenceAcquisitionQuery>(value) {
                        Ok(query) => query,
                        Err(error) => {
                            emit_error(&format!(
                                "invalid evidence-acquisition query JSON: {error}"
                            ));
                            std::process::exit(2);
                        }
                    },
                    Err(error) => {
                        emit_error(&error);
                        std::process::exit(2);
                    }
                },
                None => EvidenceAcquisitionQuery::default(),
            };
            let case_asset_report = match case_asset_manifest_path.as_deref() {
                Some(path) => {
                    let manifest = match read_case_asset_manifest(path) {
                        Ok(manifest) => manifest,
                        Err(error) => {
                            emit_error(&error);
                            std::process::exit(2);
                        }
                    };
                    let asset_query = match case_asset_manifest_query_path.as_deref() {
                        Some(query_path) => match read_query_value(query_path) {
                            Ok(value) => {
                                match serde_json::from_value::<CaseAssetManifestQuery>(value) {
                                    Ok(query) => query,
                                    Err(error) => {
                                        emit_error(&format!(
                                            "invalid case-asset-manifest query JSON: {error}"
                                        ));
                                        std::process::exit(2);
                                    }
                                }
                            }
                            Err(error) => {
                                emit_error(&error);
                                std::process::exit(2);
                            }
                        },
                        None => CaseAssetManifestQuery::default(),
                    };
                    match agent.case_asset_manifest(&request, &manifest, &asset_query) {
                        Ok(report) => Some(report),
                        Err(error) => {
                            emit_error(&error.to_string());
                            std::process::exit(2);
                        }
                    }
                }
                None => None,
            };
            let case_asset_disposition = match autonomous_acquisition_case_asset_disposition_path
                .as_deref()
            {
                Some(path) => match std::fs::read_to_string(path) {
                    Ok(text) => {
                        match serde_json::from_str::<CaseAssetReviewDispositionReport>(&text) {
                            Ok(report) => Some(report),
                            Err(error) => {
                                emit_error(&format!(
                                "invalid acquisition case-asset disposition report JSON: {error}"
                            ));
                                std::process::exit(2);
                            }
                        }
                    }
                    Err(error) => {
                        emit_error(&format!(
                            "could not read acquisition case-asset disposition report {path:?}: {error}"
                        ));
                        std::process::exit(2);
                    }
                },
                None => None,
            };
            let result = match autonomous_acquisition_operation {
                "compile" => (match case_asset_disposition.as_ref() {
                    Some(disposition) => agent
                        .evidence_acquisition_with_case_assets_and_dispositions(
                            &request,
                            real_data.as_ref(),
                            public_literature.as_ref(),
                            case_asset_report.as_ref(),
                            disposition,
                            &query,
                        ),
                    None => agent.evidence_acquisition_with_case_assets(
                        &request,
                        real_data.as_ref(),
                        public_literature.as_ref(),
                        case_asset_report.as_ref(),
                        &query,
                    ),
                })
                .map(|report| {
                    serde_json::to_value(report).expect("acquisition report is serialisable")
                }),
                "start" => (match case_asset_disposition.as_ref() {
                    Some(disposition) => agent
                        .evidence_acquisition_start_with_case_assets_and_dispositions(
                            &request,
                            real_data.as_ref(),
                            public_literature.as_ref(),
                            case_asset_report.as_ref(),
                            disposition,
                            &query,
                        ),
                    None => agent.evidence_acquisition_start_with_case_assets(
                        &request,
                        real_data.as_ref(),
                        public_literature.as_ref(),
                        case_asset_report.as_ref(),
                        &query,
                    ),
                })
                .map(|report| {
                    serde_json::to_value(report).expect("acquisition start is serialisable")
                }),
                "advance" => {
                    let path = autonomous_acquisition_session_path
                        .as_deref()
                        .expect("validated acquisition session path");
                    let session_value = match read_query_value(path) {
                        Ok(value) => value,
                        Err(error) => {
                            emit_error(&error);
                            std::process::exit(2);
                        }
                    };
                    let session: EvidenceAcquisitionSession =
                        match serde_json::from_value(session_value) {
                            Ok(session) => session,
                            Err(error) => {
                                emit_error(&format!(
                                    "invalid evidence-acquisition session JSON: {error}"
                                ));
                                std::process::exit(2);
                            }
                        };
                    let advanced = match case_asset_disposition.as_ref() {
                        Some(disposition) => agent
                            .evidence_acquisition_advance_with_case_assets_and_dispositions(
                                &session,
                                &request,
                                real_data.as_ref(),
                                public_literature.as_ref(),
                                case_asset_report.as_ref(),
                                disposition,
                                &query,
                                autonomous_acquisition_max_steps,
                            ),
                        None => agent.evidence_acquisition_advance_with_case_assets(
                            &session,
                            &request,
                            real_data.as_ref(),
                            public_literature.as_ref(),
                            case_asset_report.as_ref(),
                            &query,
                            autonomous_acquisition_max_steps,
                        ),
                    };
                    advanced.map(|report| {
                        serde_json::to_value(report).expect("acquisition advance is serialisable")
                    })
                }
                "finish" => {
                    let path = autonomous_acquisition_session_path
                        .as_deref()
                        .expect("validated acquisition session path");
                    let session_value = match read_query_value(path) {
                        Ok(value) => value,
                        Err(error) => {
                            emit_error(&error);
                            std::process::exit(2);
                        }
                    };
                    let session: EvidenceAcquisitionSession =
                        match serde_json::from_value(session_value) {
                            Ok(session) => session,
                            Err(error) => {
                                emit_error(&format!(
                                    "invalid evidence-acquisition session JSON: {error}"
                                ));
                                std::process::exit(2);
                            }
                        };
                    let finished = match case_asset_disposition.as_ref() {
                        Some(disposition) => agent
                            .evidence_acquisition_finish_with_case_assets_and_dispositions(
                                &session,
                                &request,
                                real_data.as_ref(),
                                public_literature.as_ref(),
                                case_asset_report.as_ref(),
                                disposition,
                                &query,
                            ),
                        None => agent.evidence_acquisition_finish_with_case_assets(
                            &session,
                            &request,
                            real_data.as_ref(),
                            public_literature.as_ref(),
                            case_asset_report.as_ref(),
                            &query,
                        ),
                    };
                    finished.map(|report| {
                        serde_json::to_value(report).expect("acquisition finish is serialisable")
                    })
                }
                _ => unreachable!("validated acquisition operation"),
            };
            match result {
                Ok(value) => println!(
                    "{}",
                    serde_json::to_string_pretty(&value)
                        .expect("evidence acquisition result is serialisable")
                ),
                Err(error) => {
                    emit_error(&error.to_string());
                    std::process::exit(2);
                }
            }
        } else {
            match agent.plan_research(
                &request,
                real_data.as_ref(),
                public_literature.as_ref(),
                max_research_plan_tasks,
                max_research_plan_references,
            ) {
                Ok(report) => println!(
                    "{}",
                    serde_json::to_string_pretty(&report).expect("research plan is serialisable")
                ),
                Err(error) => {
                    emit_error(&error.to_string());
                    std::process::exit(2);
                }
            }
        }
        return;
    }

    if audit_evidence {
        let request: CaseRequest = match serde_json::from_value(document) {
            Ok(request) => request,
            Err(error) => {
                emit_error(&format!("invalid neurosurgical request JSON: {error}"));
                std::process::exit(2);
            }
        };
        let report = if temporal_audit {
            agent
                .temporal_audit(&request)
                .map(|report| serde_json::to_value(report).expect("temporal audit is serialisable"))
        } else {
            agent
                .audit_evidence(&request)
                .map(|report| serde_json::to_value(report).expect("evidence audit is serialisable"))
        };
        match report {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report).expect("audit report is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if specialty_evidence_map {
        let request: CaseRequest = match serde_json::from_value(document) {
            Ok(request) => request,
            Err(error) => {
                emit_error(&format!("invalid neurosurgical request JSON: {error}"));
                std::process::exit(2);
            }
        };
        match agent.specialty_evidence_map(&request) {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("specialty evidence map is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if evidence_synthesis {
        let request: CaseRequest = match serde_json::from_value(document) {
            Ok(request) => request,
            Err(error) => {
                emit_error(&format!("invalid neurosurgical request JSON: {error}"));
                std::process::exit(2);
            }
        };
        let query = match evidence_synthesis_query_path.as_deref() {
            Some(path) => match read_query_value(path) {
                Ok(value) => match serde_json::from_value::<EvidenceSynthesisQuery>(value) {
                    Ok(query) => query,
                    Err(error) => {
                        emit_error(&format!("invalid evidence-synthesis query JSON: {error}"));
                        std::process::exit(2);
                    }
                },
                Err(error) => {
                    emit_error(&error);
                    std::process::exit(2);
                }
            },
            None => EvidenceSynthesisQuery::default(),
        };
        let case_asset_report = match case_asset_manifest_path.as_deref() {
            Some(path) => {
                let manifest = match read_case_asset_manifest(path) {
                    Ok(manifest) => manifest,
                    Err(error) => {
                        emit_error(&error);
                        std::process::exit(2);
                    }
                };
                let asset_query = match case_asset_manifest_query_path.as_deref() {
                    Some(query_path) => match read_query_value(query_path) {
                        Ok(value) => {
                            match serde_json::from_value::<CaseAssetManifestQuery>(value) {
                                Ok(query) => query,
                                Err(error) => {
                                    emit_error(&format!(
                                        "invalid case-asset-manifest query JSON: {error}"
                                    ));
                                    std::process::exit(2);
                                }
                            }
                        }
                        Err(error) => {
                            emit_error(&error);
                            std::process::exit(2);
                        }
                    },
                    None => CaseAssetManifestQuery::default(),
                };
                match agent.case_asset_manifest(&request, &manifest, &asset_query) {
                    Ok(report) => Some(report),
                    Err(error) => {
                        emit_error(&error.to_string());
                        std::process::exit(2);
                    }
                }
            }
            None => None,
        };
        match agent.evidence_synthesis_with_case_assets(
            &request,
            real_data.as_ref(),
            public_literature.as_ref(),
            &query,
            case_asset_report.as_ref(),
        ) {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report).expect("evidence synthesis is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if glioma_molecular_map {
        let request: CaseRequest = match serde_json::from_value(document) {
            Ok(request) => request,
            Err(error) => {
                emit_error(&format!(
                    "invalid glioma molecular-map request JSON: {error}"
                ));
                std::process::exit(2);
            }
        };
        let query = match glioma_molecular_map_query_path.as_deref() {
            Some(path) => match read_query_value(path) {
                Ok(value) => match serde_json::from_value::<GliomaMolecularMapQuery>(value) {
                    Ok(query) => query,
                    Err(error) => {
                        emit_error(&format!("invalid glioma molecular-map query JSON: {error}"));
                        std::process::exit(2);
                    }
                },
                Err(error) => {
                    emit_error(&error);
                    std::process::exit(2);
                }
            },
            None => GliomaMolecularMapQuery::default(),
        };
        match agent.glioma_molecular_map(
            &request,
            real_data.as_ref(),
            public_literature.as_ref(),
            &query,
        ) {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("glioma molecular evidence map is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if case_fhir_import_mode {
        let path = case_fhir_import_path
            .as_deref()
            .expect("case_fhir_import_mode implies an import path");
        let request: CaseRequest = match serde_json::from_value(document) {
            Ok(request) => request,
            Err(error) => {
                emit_error(&format!("invalid FHIR import CaseRequest JSON: {error}"));
                std::process::exit(2);
            }
        };
        let import = match read_fhir_case_import(path) {
            Ok(import) => import,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let report: FhirCaseImportReport = match agent.case_fhir_import(&request, &import) {
            Ok(report) => report,
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("FHIR import report is serialisable")
        );
        return;
    }

    if case_dicom_import_mode {
        let path = case_dicom_import_path
            .as_deref()
            .expect("case_dicom_import_mode implies an import path");
        let request: CaseRequest = match serde_json::from_value(document) {
            Ok(request) => request,
            Err(error) => {
                emit_error(&format!("invalid DICOM import CaseRequest JSON: {error}"));
                std::process::exit(2);
            }
        };
        let import = match read_dicom_case_import(path) {
            Ok(import) => import,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let report: DicomCaseImportReport = match agent.case_dicom_import(&request, &import) {
            Ok(report) => report,
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("DICOM import report is serialisable")
        );
        return;
    }

    if case_dicom_evidence_workflow_mode {
        let path = case_dicom_evidence_workflow_path
            .as_deref()
            .expect("case_dicom_evidence_workflow_mode implies an import path");
        let request: CaseRequest = match serde_json::from_value(document) {
            Ok(request) => request,
            Err(error) => {
                emit_error(&format!(
                    "invalid DICOM evidence workflow CaseRequest JSON: {error}"
                ));
                std::process::exit(2);
            }
        };
        let import = match read_dicom_case_import(path) {
            Ok(import) => import,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let query = match case_dicom_evidence_workflow_query_path.as_deref() {
            Some(query_path) => match read_query_value(query_path) {
                Ok(value) => match serde_json::from_value::<DicomEvidenceWorkflowQuery>(value) {
                    Ok(query) => query,
                    Err(error) => {
                        emit_error(&format!(
                            "invalid DICOM evidence workflow query JSON: {error}"
                        ));
                        std::process::exit(2);
                    }
                },
                Err(error) => {
                    emit_error(&error);
                    std::process::exit(2);
                }
            },
            None => DicomEvidenceWorkflowQuery::default(),
        };
        let report: DicomEvidenceWorkflowReport = match agent.case_dicom_evidence_workflow(
            &request,
            &import,
            real_data.as_ref(),
            public_literature.as_ref(),
            &query,
        ) {
            Ok(report) => report,
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .expect("DICOM evidence workflow report is serialisable")
        );
        return;
    }

    if case_asset_manifest_mode {
        let path = case_asset_manifest_path
            .as_deref()
            .expect("case_asset_manifest_mode implies a manifest path");
        let request: CaseRequest = match serde_json::from_value(document) {
            Ok(request) => request,
            Err(error) => {
                emit_error(&format!(
                    "invalid case-asset-manifest request JSON: {error}"
                ));
                std::process::exit(2);
            }
        };
        let manifest = match read_case_asset_manifest(path) {
            Ok(manifest) => manifest,
            Err(error) => {
                emit_error(&error);
                std::process::exit(2);
            }
        };
        let query = match case_asset_manifest_query_path.as_deref() {
            Some(query_path) => match read_query_value(query_path) {
                Ok(value) => match serde_json::from_value::<CaseAssetManifestQuery>(value) {
                    Ok(query) => query,
                    Err(error) => {
                        emit_error(&format!("invalid case-asset-manifest query JSON: {error}"));
                        std::process::exit(2);
                    }
                },
                Err(error) => {
                    emit_error(&error);
                    std::process::exit(2);
                }
            },
            None => CaseAssetManifestQuery::default(),
        };
        match agent.case_asset_manifest(&request, &manifest, &query) {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("case asset manifest report is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if research_brief {
        let request: CaseRequest = match serde_json::from_value(document) {
            Ok(request) => request,
            Err(error) => {
                emit_error(&format!("invalid neurosurgical request JSON: {error}"));
                std::process::exit(2);
            }
        };
        let query = match research_brief_query_path.as_deref() {
            Some(path) => match read_query_value(path) {
                Ok(value) => match serde_json::from_value::<NeurosurgicalResearchBriefQuery>(value)
                {
                    Ok(query) => query,
                    Err(error) => {
                        emit_error(&format!("invalid research-brief query JSON: {error}"));
                        std::process::exit(2);
                    }
                },
                Err(error) => {
                    emit_error(&error);
                    std::process::exit(2);
                }
            },
            None => NeurosurgicalResearchBriefQuery::default(),
        };
        let result = match (real_data.as_ref(), public_literature.as_ref()) {
            (Some(data), None) => agent.research_brief(&request, Some(data), None, &query),
            (None, Some(literature)) => {
                agent.research_brief(&request, None, Some(literature), &query)
            }
            (None, None) => Err(bioprism_neurosurgery::NeurosurgeryError::RealDataRejected {
                reason: "--research-brief requires --real-glioma or --public-literature"
                    .to_string(),
            }),
            (Some(_), Some(_)) => Err(bioprism_neurosurgery::NeurosurgeryError::RealDataRejected {
                reason: "--research-brief accepts one evidence bundle, not both".to_string(),
            }),
        };
        match result {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report).expect("research brief is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if evidence_program {
        let request: CaseRequest = match serde_json::from_value(document) {
            Ok(request) => request,
            Err(error) => {
                emit_error(&format!("invalid evidence-program request JSON: {error}"));
                std::process::exit(2);
            }
        };
        let query = match evidence_program_query_path.as_deref() {
            Some(path) => match read_query_value(path) {
                Ok(value) => match serde_json::from_value::<EvidenceProgramQuery>(value) {
                    Ok(query) => query,
                    Err(error) => {
                        emit_error(&format!("invalid evidence-program query JSON: {error}"));
                        std::process::exit(2);
                    }
                },
                Err(error) => {
                    emit_error(&error);
                    std::process::exit(2);
                }
            },
            None => EvidenceProgramQuery::default(),
        };
        let case_asset_manifest = match case_asset_manifest_path.as_deref() {
            Some(path) => match read_case_asset_manifest(path) {
                Ok(manifest) => Some(manifest),
                Err(error) => {
                    emit_error(&error);
                    std::process::exit(2);
                }
            },
            None => None,
        };
        let case_asset_query = match case_asset_manifest_query_path.as_deref() {
            Some(path) => match read_query_value(path) {
                Ok(value) => match serde_json::from_value::<CaseAssetManifestQuery>(value) {
                    Ok(query) => Some(query),
                    Err(error) => {
                        emit_error(&format!("invalid case-asset-manifest query JSON: {error}"));
                        std::process::exit(2);
                    }
                },
                Err(error) => {
                    emit_error(&error);
                    std::process::exit(2);
                }
            },
            None => None,
        };
        let result = match case_asset_manifest.as_ref() {
            Some(manifest) => agent.evidence_program_with_case_assets(
                &request,
                real_data.as_ref(),
                public_literature.as_ref(),
                manifest,
                &case_asset_query.unwrap_or_default(),
                &query,
            ),
            None => agent.evidence_program(
                &request,
                real_data.as_ref(),
                public_literature.as_ref(),
                &query,
            ),
        };
        match result {
            Ok(report) => println!(
                "{}",
                serde_json::to_string_pretty(&report).expect("evidence program is serialisable")
            ),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    if session_start
        || session_advance.is_some()
        || session_finish.is_some()
        || session_run
        || mission
    {
        let request: CaseRequest = match serde_json::from_value(document) {
            Ok(request) => request,
            Err(error) => {
                emit_error(&format!("invalid neurosurgical request JSON: {error}"));
                std::process::exit(2);
            }
        };
        let mission_query = match mission_query_path.as_deref() {
            Some(path) => match read_query_value(path) {
                Ok(query) => Some(query),
                Err(error) => {
                    emit_error(&error);
                    std::process::exit(2);
                }
            },
            None => None,
        };
        let mission_public_literature_query = match mission_public_literature_query_path.as_deref()
        {
            Some(path) => match read_query_value(path) {
                Ok(query) => Some(query),
                Err(error) => {
                    emit_error(&error);
                    std::process::exit(2);
                }
            },
            None => None,
        };
        let mission_portfolio_query = match mission_portfolio_query_path.as_deref() {
            Some(path) => match read_query_value(path) {
                Ok(query) => Some(query),
                Err(error) => {
                    emit_error(&error);
                    std::process::exit(2);
                }
            },
            None => None,
        };
        if mission_portfolio_query.is_some() && public_literature.is_none() {
            emit_error("--mission-portfolio-query requires --public-literature");
            std::process::exit(2);
        }
        if mission_public_literature_query.is_some() && public_literature.is_none() {
            emit_error("--mission-public-literature-query requires --public-literature");
            std::process::exit(2);
        }
        let mission_freshness = match mission_freshness_path.as_deref() {
            Some(path) => match read_query_value(path) {
                Ok(query) => Some(query),
                Err(error) => {
                    emit_error(&error);
                    std::process::exit(2);
                }
            },
            None => None,
        };
        let case_asset_manifest = match case_asset_manifest_path.as_deref() {
            Some(path) => match read_case_asset_manifest(path) {
                Ok(manifest) => Some(manifest),
                Err(error) => {
                    emit_error(&error);
                    std::process::exit(2);
                }
            },
            None => None,
        };
        let case_asset_query = match case_asset_manifest_query_path.as_deref() {
            Some(path) => match read_query_value(path) {
                Ok(value) => match serde_json::from_value::<CaseAssetManifestQuery>(value) {
                    Ok(query) => Some(query),
                    Err(error) => {
                        emit_error(&format!("invalid case-asset-manifest query JSON: {error}"));
                        std::process::exit(2);
                    }
                },
                Err(error) => {
                    emit_error(&error);
                    std::process::exit(2);
                }
            },
            None => None,
        };
        let mission_case_dicom = match mission_case_dicom_path.as_deref() {
            Some(path) => match read_dicom_case_import(path) {
                Ok(import) => Some(import),
                Err(error) => {
                    emit_error(&error);
                    std::process::exit(2);
                }
            },
            None => None,
        };
        let mission_case_fhir = match mission_case_fhir_path.as_deref() {
            Some(path) => match read_fhir_case_import(path) {
                Ok(import) => Some(import),
                Err(error) => {
                    emit_error(&error);
                    std::process::exit(2);
                }
            },
            None => None,
        };
        let mission_case_asset_disposition = match mission_case_asset_review_disposition_path
            .as_deref()
        {
            Some(path) => match std::fs::read_to_string(path) {
                Ok(text) => match serde_json::from_str::<CaseAssetReviewDispositionReport>(&text) {
                    Ok(report) => Some(report),
                    Err(error) => {
                        emit_error(&format!(
                            "invalid mission case-asset disposition report JSON: {error}"
                        ));
                        std::process::exit(2);
                    }
                },
                Err(error) => {
                    emit_error(&format!(
                        "could not read mission case-asset disposition report {path:?}: {error}"
                    ));
                    std::process::exit(2);
                }
            },
            None => None,
        };
        let result = if mission {
            if mission_case_fhir.is_some() && mission_case_dicom.is_some() {
                if case_asset_manifest.is_some()
                    || case_asset_query.is_some()
                    || mission_case_asset_disposition.is_some()
                {
                    Err(bioprism_neurosurgery::NeurosurgeryError::RealDataRejected {
                        reason: "--mission-case-dicom and --mission-case-fhir cannot be combined with case-asset manifest/query/disposition options".to_string(),
                    })
                } else if real_data.is_none() {
                    Err(bioprism_neurosurgery::NeurosurgeryError::RealDataRejected {
                        reason: "--mission-case-dicom requires --real-glioma even when composed with FHIR".to_string(),
                    })
                } else {
                    let query = mission_query
                        .as_ref()
                        .map(|value| serde_json::from_value::<RealDataQuery>(value.clone()))
                        .transpose()
                        .map_err(|error| {
                            bioprism_neurosurgery::NeurosurgeryError::Json(error.to_string())
                        });
                    let public_query = mission_public_literature_query
                        .as_ref()
                        .map(|value| serde_json::from_value::<PublicLiteratureQuery>(value.clone()))
                        .transpose()
                        .map_err(|error| {
                            bioprism_neurosurgery::NeurosurgeryError::Json(error.to_string())
                        });
                    let freshness = mission_freshness
                        .as_ref()
                        .map(|value| {
                            serde_json::from_value::<RealDataFreshnessQuery>(value.clone())
                        })
                        .transpose()
                        .map_err(|error| {
                            bioprism_neurosurgery::NeurosurgeryError::Json(error.to_string())
                        });
                    let portfolio_query = mission_portfolio_query
                        .as_ref()
                        .map(|value| {
                            serde_json::from_value::<PublicLiteraturePortfolioQuery>(value.clone())
                        })
                        .transpose()
                        .map_err(|error| {
                            bioprism_neurosurgery::NeurosurgeryError::Json(error.to_string())
                        });
                    query.and_then(|query| {
                        public_query.and_then(|public_query| {
                            freshness.and_then(|freshness| {
                                portfolio_query.and_then(|portfolio_query| {
                                    agent
                                        .run_research_mission_with_case_imports(
                                            &request,
                                            real_data.as_ref(),
                                            public_literature.as_ref(),
                                            query.as_ref(),
                                            public_query.as_ref(),
                                            freshness.as_ref(),
                                            portfolio_query.as_ref(),
                                            mission_case_dicom.as_ref(),
                                            mission_case_fhir.as_ref(),
                                            max_session_steps,
                                        )
                                        .map(|result| {
                                            serde_json::to_value(result)
                                                .expect("mission is serialisable")
                                        })
                                })
                            })
                        })
                    })
                }
            } else if let Some(fhir_import) = mission_case_fhir.as_ref() {
                if case_asset_manifest.is_some()
                    || case_asset_query.is_some()
                    || mission_case_asset_disposition.is_some()
                {
                    Err(bioprism_neurosurgery::NeurosurgeryError::RealDataRejected {
                        reason: "--mission-case-fhir cannot be combined with case-asset manifest/query/disposition options".to_string(),
                    })
                } else {
                    let query = mission_query
                        .as_ref()
                        .filter(|_| real_data.is_some())
                        .map(|value| serde_json::from_value::<RealDataQuery>(value.clone()))
                        .transpose()
                        .map_err(|error| {
                            bioprism_neurosurgery::NeurosurgeryError::Json(error.to_string())
                        });
                    let public_query = mission_public_literature_query
                        .as_ref()
                        .or_else(|| {
                            if real_data.is_none() && public_literature.is_some() {
                                mission_query.as_ref()
                            } else {
                                None
                            }
                        })
                        .map(|value| serde_json::from_value::<PublicLiteratureQuery>(value.clone()))
                        .transpose()
                        .map_err(|error| {
                            bioprism_neurosurgery::NeurosurgeryError::Json(error.to_string())
                        });
                    let freshness = mission_freshness
                        .as_ref()
                        .map(|value| {
                            serde_json::from_value::<RealDataFreshnessQuery>(value.clone())
                        })
                        .transpose()
                        .map_err(|error| {
                            bioprism_neurosurgery::NeurosurgeryError::Json(error.to_string())
                        });
                    let portfolio_query = mission_portfolio_query
                        .as_ref()
                        .map(|value| {
                            serde_json::from_value::<PublicLiteraturePortfolioQuery>(value.clone())
                        })
                        .transpose()
                        .map_err(|error| {
                            bioprism_neurosurgery::NeurosurgeryError::Json(error.to_string())
                        });
                    query.and_then(|query| {
                        public_query.and_then(|public_query| {
                            freshness.and_then(|freshness| {
                                portfolio_query.and_then(|portfolio_query| {
                                    agent
                                        .run_research_mission_with_case_fhir(
                                            &request,
                                            real_data.as_ref(),
                                            public_literature.as_ref(),
                                            query.as_ref(),
                                            public_query.as_ref(),
                                            freshness.as_ref(),
                                            portfolio_query.as_ref(),
                                            fhir_import,
                                            max_session_steps,
                                        )
                                        .map(|result| {
                                            serde_json::to_value(result)
                                                .expect("mission is serialisable")
                                        })
                                })
                            })
                        })
                    })
                }
            } else {
                if let Some(dicom_import) = mission_case_dicom.as_ref() {
                    if public_literature.is_some() {
                        Err(bioprism_neurosurgery::NeurosurgeryError::RealDataRejected {
                        reason: "--mission-case-dicom currently supports a real-glioma-only mission; remove --public-literature".to_string(),
                    })
                    } else if case_asset_manifest.is_some()
                        || case_asset_query.is_some()
                        || mission_case_asset_disposition.is_some()
                    {
                        Err(bioprism_neurosurgery::NeurosurgeryError::RealDataRejected {
                        reason: "--mission-case-dicom cannot be combined with case-asset manifest/query/disposition options".to_string(),
                    })
                    } else if let Some(real_data) = real_data.as_ref() {
                        let query = mission_query
                            .as_ref()
                            .map(|value| serde_json::from_value::<RealDataQuery>(value.clone()))
                            .transpose()
                            .map_err(|error| {
                                bioprism_neurosurgery::NeurosurgeryError::Json(error.to_string())
                            });
                        let freshness = mission_freshness
                            .as_ref()
                            .map(|value| {
                                serde_json::from_value::<RealDataFreshnessQuery>(value.clone())
                            })
                            .transpose()
                            .map_err(|error| {
                                bioprism_neurosurgery::NeurosurgeryError::Json(error.to_string())
                            });
                        query.and_then(|query| {
                            freshness.and_then(|freshness| {
                                agent
                                    .run_research_mission_with_case_dicom(
                                        &request,
                                        real_data,
                                        query.as_ref(),
                                        freshness.as_ref(),
                                        dicom_import,
                                        max_session_steps,
                                    )
                                    .map(|result| {
                                        serde_json::to_value(result)
                                            .expect("mission is serialisable")
                                    })
                            })
                        })
                    } else {
                        Err(bioprism_neurosurgery::NeurosurgeryError::RealDataRejected {
                            reason: "--mission-case-dicom requires --real-glioma".to_string(),
                        })
                    }
                } else if let (Some(real_data), Some(literature)) =
                    (real_data.as_ref(), public_literature.as_ref())
                {
                    let query = mission_query
                        .as_ref()
                        .map(|value| serde_json::from_value::<RealDataQuery>(value.clone()))
                        .transpose()
                        .map_err(|error| {
                            bioprism_neurosurgery::NeurosurgeryError::Json(error.to_string())
                        });
                    let public_query = mission_public_literature_query
                        .as_ref()
                        .map(|value| serde_json::from_value::<PublicLiteratureQuery>(value.clone()))
                        .transpose()
                        .map_err(|error| {
                            bioprism_neurosurgery::NeurosurgeryError::Json(error.to_string())
                        });
                    let freshness = mission_freshness
                        .as_ref()
                        .map(|value| {
                            serde_json::from_value::<RealDataFreshnessQuery>(value.clone())
                        })
                        .transpose()
                        .map_err(|error| {
                            bioprism_neurosurgery::NeurosurgeryError::Json(error.to_string())
                        });
                    query.and_then(|query| {
                    public_query.and_then(|public_query| {
                        freshness.and_then(|freshness| {
                            let portfolio_query = mission_portfolio_query
                                .as_ref()
                                .map(|value| {
                                    serde_json::from_value::<PublicLiteraturePortfolioQuery>(
                                        value.clone(),
                                    )
                                })
                                .transpose()
                                .map_err(|error| {
                                    bioprism_neurosurgery::NeurosurgeryError::Json(
                                        error.to_string(),
                                    )
                                });
                            portfolio_query.and_then(|portfolio_query| {
                                agent
                                    .run_research_mission_with_real_data_and_public_literature_case_assets_and_dispositions(
                                        &request,
                                        real_data,
                                        literature,
                                        query.as_ref(),
                                        public_query.as_ref(),
                                        freshness.as_ref(),
                                        portfolio_query.as_ref(),
                                        case_asset_manifest.as_ref(),
                                        case_asset_query.as_ref(),
                                        mission_case_asset_disposition.as_ref(),
                                        max_session_steps,
                                    )
                                    .map(|result| {
                                        serde_json::to_value(result)
                                            .expect("mission is serialisable")
                                    })
                            })
                        })
                    })
                })
                } else if let Some(literature) = public_literature.as_ref() {
                    let query = mission_public_literature_query
                        .as_ref()
                        .or(mission_query.as_ref())
                        .map(|value| serde_json::from_value::<PublicLiteratureQuery>(value.clone()))
                        .transpose()
                        .map_err(|error| {
                            bioprism_neurosurgery::NeurosurgeryError::Json(error.to_string())
                        });
                    let freshness = mission_freshness
                        .as_ref()
                        .map(|value| {
                            serde_json::from_value::<RealDataFreshnessQuery>(value.clone())
                        })
                        .transpose()
                        .map_err(|error| {
                            bioprism_neurosurgery::NeurosurgeryError::Json(error.to_string())
                        });
                    query.and_then(|query| {
                    freshness.and_then(|freshness| {
                        let portfolio_query = mission_portfolio_query
                            .as_ref()
                            .map(|value| {
                                serde_json::from_value::<PublicLiteraturePortfolioQuery>(
                                    value.clone(),
                                )
                            })
                            .transpose()
                            .map_err(|error| {
                                bioprism_neurosurgery::NeurosurgeryError::Json(error.to_string())
                            });
                        portfolio_query.and_then(|portfolio_query| {
                            agent
                                .run_research_mission_with_public_literature_case_assets_and_dispositions(
                                    &request,
                                    literature,
                                    query.as_ref(),
                                    freshness.as_ref(),
                                    portfolio_query.as_ref(),
                                    case_asset_manifest.as_ref(),
                                    case_asset_query.as_ref(),
                                    mission_case_asset_disposition.as_ref(),
                                    max_session_steps,
                                )
                                .map(|result| {
                                    serde_json::to_value(result).expect("mission is serialisable")
                                })
                        })
                    })
                })
                } else {
                    let query = mission_query
                        .as_ref()
                        .map(|value| serde_json::from_value::<RealDataQuery>(value.clone()))
                        .transpose()
                        .map_err(|error| {
                            bioprism_neurosurgery::NeurosurgeryError::Json(error.to_string())
                        });
                    let freshness = mission_freshness
                        .as_ref()
                        .map(|value| {
                            serde_json::from_value::<RealDataFreshnessQuery>(value.clone())
                        })
                        .transpose()
                        .map_err(|error| {
                            bioprism_neurosurgery::NeurosurgeryError::Json(error.to_string())
                        });
                    query.and_then(|query| {
                        freshness.and_then(|freshness| {
                            agent
                                .run_research_mission_with_case_assets_and_dispositions(
                                    &request,
                                    real_data.as_ref(),
                                    query.as_ref(),
                                    freshness.as_ref(),
                                    case_asset_manifest.as_ref(),
                                    case_asset_query.as_ref(),
                                    mission_case_asset_disposition.as_ref(),
                                    max_session_steps,
                                )
                                .map(|result| {
                                    serde_json::to_value(result).expect("mission is serialisable")
                                })
                        })
                    })
                }
            }
        } else if session_start {
            if let Some(literature) = public_literature.as_ref() {
                agent
                    .start_session_with_public_literature(&request, literature)
                    .map(|session| serde_json::to_value(session).expect("session is serialisable"))
            } else {
                agent
                    .start_session(&request, real_data.as_ref())
                    .map(|session| serde_json::to_value(session).expect("session is serialisable"))
            }
        } else if session_run {
            if let Some(literature) = public_literature.as_ref() {
                agent
                    .run_session_to_review_with_public_literature(
                        &request,
                        literature,
                        max_session_steps,
                    )
                    .map(|result| {
                        serde_json::to_value(result).expect("session run is serialisable")
                    })
            } else {
                agent
                    .run_session_to_review(&request, real_data.as_ref(), max_session_steps)
                    .map(|result| {
                        serde_json::to_value(result).expect("session run is serialisable")
                    })
            }
        } else {
            let path = session_advance
                .as_deref()
                .or(session_finish.as_deref())
                .unwrap();
            let session = match read_session(path) {
                Ok(session) => session,
                Err(error) => {
                    emit_error(&error);
                    std::process::exit(2);
                }
            };
            if session_advance.is_some() {
                if let Some(literature) = public_literature.as_ref() {
                    agent
                        .advance_session_with_public_literature(&session, &request, literature)
                        .map(|session| {
                            serde_json::to_value(session).expect("session is serialisable")
                        })
                } else {
                    agent
                        .advance_session(&session, &request, real_data.as_ref())
                        .map(|session| {
                            serde_json::to_value(session).expect("session is serialisable")
                        })
                }
            } else {
                if let Some(literature) = public_literature.as_ref() {
                    agent
                        .finish_session_with_public_literature(&session, &request, literature)
                        .map(|response| {
                            serde_json::to_value(response).expect("response is serialisable")
                        })
                } else {
                    agent
                        .finish_session(&session, &request, real_data.as_ref())
                        .map(|response| {
                            serde_json::to_value(response).expect("response is serialisable")
                        })
                }
            }
        };
        match result {
            Ok(value) => println!("{}", serde_json::to_string_pretty(&value).expect("JSON")),
            Err(error) => {
                emit_error(&error.to_string());
                std::process::exit(2);
            }
        }
        return;
    }

    let result = if real_data_path.is_some() {
        let data = real_data.expect("real-data path was parsed");
        match serde_json::from_value::<CaseRequest>(document) {
            Ok(request) => agent.run_with_real_glioma_data(&request, &data),
            Err(error) => Err(bioprism_neurosurgery::NeurosurgeryError::Json(
                error.to_string(),
            )),
        }
    } else if let Some(literature) = public_literature.as_ref() {
        match serde_json::from_value::<CaseRequest>(document) {
            Ok(request) => agent.run_with_public_literature(&request, literature),
            Err(error) => Err(bioprism_neurosurgery::NeurosurgeryError::Json(
                error.to_string(),
            )),
        }
    } else {
        agent.run_json(&document)
    };
    match result {
        Ok(response) => println!(
            "{}",
            serde_json::to_string_pretty(&response).expect("response is serialisable")
        ),
        Err(error) => {
            emit_error(&error.to_string());
            std::process::exit(2);
        }
    }
}

fn argument_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

fn argument_pair(args: &[String], name: &str) -> Option<(String, String)> {
    let index = args.iter().position(|arg| arg == name)?;
    let before = args.get(index + 1).cloned().unwrap_or_else(|| {
        emit_error(&format!("{name} requires a before path and an after path"));
        std::process::exit(2);
    });
    let after = args.get(index + 2).cloned().unwrap_or_else(|| {
        emit_error(&format!("{name} requires a before path and an after path"));
        std::process::exit(2);
    });
    Some((before, after))
}

fn read_real_data(path: &str) -> Result<RealGliomaBundle, String> {
    let data_text = std::fs::read_to_string(path)
        .map_err(|error| format!("could not read real glioma data bundle {path:?}: {error}"))?;
    serde_json::from_str(&data_text)
        .map_err(|error| format!("invalid real glioma data JSON: {error}"))
}

fn read_public_literature(path: &str) -> Result<PublicLiteratureBundle, String> {
    let data_text = std::fs::read_to_string(path)
        .map_err(|error| format!("could not read public-literature bundle {path:?}: {error}"))?;
    serde_json::from_str(&data_text)
        .map_err(|error| format!("invalid public-literature bundle JSON: {error}"))
}

fn read_case_asset_manifest(path: &str) -> Result<CaseAssetManifest, String> {
    let manifest_text = std::fs::read_to_string(path)
        .map_err(|error| format!("could not read case asset manifest {path:?}: {error}"))?;
    serde_json::from_str(&manifest_text)
        .map_err(|error| format!("invalid case asset manifest JSON: {error}"))
}

fn read_fhir_case_import(path: &str) -> Result<FhirCaseImport, String> {
    let import_text = std::fs::read_to_string(path)
        .map_err(|error| format!("could not read FHIR import document {path:?}: {error}"))?;
    serde_json::from_str(&import_text).map_err(|error| format!("invalid FHIR import JSON: {error}"))
}

fn read_dicom_case_import(path: &str) -> Result<DicomCaseImport, String> {
    let import_text = std::fs::read_to_string(path)
        .map_err(|error| format!("could not read DICOM import document {path:?}: {error}"))?;
    serde_json::from_str(&import_text)
        .map_err(|error| format!("invalid DICOM import JSON: {error}"))
}

fn read_session(path: &str) -> Result<NeurosurgicalSession, String> {
    let session_text = std::fs::read_to_string(path)
        .map_err(|error| format!("could not read neurosurgical session {path:?}: {error}"))?;
    serde_json::from_str(&session_text)
        .map_err(|error| format!("invalid neurosurgical session JSON: {error}"))
}

fn read_query_value(path: &str) -> Result<Value, String> {
    let query_text = std::fs::read_to_string(path)
        .map_err(|error| format!("could not read mission query {path:?}: {error}"))?;
    serde_json::from_str(&query_text)
        .map_err(|error| format!("invalid mission query JSON: {error}"))
}

fn emit_error(message: &str) {
    eprintln!(
        "{}",
        serde_json::json!({
            "schema_version": NEUROSURGERY_SCHEMA_VERSION,
            "error": message,
            "clinical_action": "none"
        })
    );
}
