//! Deterministic, provider-neutral orchestration for the specialty tools.

use crate::case_asset_manifest::{
    CaseAssetManifest, CaseAssetManifestQuery, CaseAssetManifestReport,
};
use crate::case_asset_review_disposition::CaseAssetReviewDispositionReport;
use crate::case_dicom::{DicomCaseImport, DicomCaseImportReport};
use crate::case_dicom_workflow::{DicomEvidenceWorkflowQuery, DicomEvidenceWorkflowReport};
use crate::case_fhir::{FhirCaseImport, FhirCaseImportReport};
use crate::catalogue::{required_capabilities, tool_catalogue, tool_spec};
use crate::error::NeurosurgeryError;
use crate::evidence_audit::{audit as audit_evidence, EvidenceAuditReport};
use crate::evidence_graph::{EvidenceGraphQuery, EvidenceGraphReport};
use crate::evidence_program::{EvidenceProgramQuery, EvidenceProgramReport};
use crate::evidence_synthesis::{EvidenceSynthesisQuery, EvidenceSynthesisReport};
use crate::glioma_molecular_map::{GliomaMolecularEvidenceMapReport, GliomaMolecularMapQuery};
use crate::intake::{
    NeurosurgicalIntakeMissionResult, NeurosurgicalIntakeMissionStatus, NeurosurgicalIntakePlan,
    NeurosurgicalIntakePortfolioQuery, NeurosurgicalIntakePortfolioResult,
    NeurosurgicalIntakeQuery,
};
use crate::literature_link::{LiteratureLinkAuditQuery, LiteratureLinkAuditReport};
use crate::model::*;
use crate::public_literature::{PublicLiteratureBundle, PublicLiteratureSummary};
use crate::public_literature_draft_audit::{
    PublicLiteratureDraftAuditReport, PublicLiteratureDraftAuditRequest,
    PublicLiteratureEvidencePacketQuery, PublicLiteratureEvidencePacketReport,
};
use crate::public_literature_integrity::{
    PublicLiteratureIntegrityAuditQuery, PublicLiteratureIntegrityAuditReport,
};
use crate::public_literature_matrix::{PublicLiteratureMatrixQuery, PublicLiteratureMatrixReport};
use crate::public_literature_portfolio::{
    PublicLiteraturePortfolioQuery, PublicLiteraturePortfolioReport,
};
use crate::public_literature_reasoning_context::{
    PublicLiteratureReasoningContextQuery, PublicLiteratureReasoningContextReport,
};
use crate::public_literature_refresh::{
    PublicLiteratureRefreshAuditQuery, PublicLiteratureRefreshAuditReport,
};
use crate::public_literature_review_queue::{
    PublicLiteratureReviewQueueQuery, PublicLiteratureReviewQueueReport,
};
use crate::public_literature_workbench::{
    PublicLiteratureWorkbenchQuery, PublicLiteratureWorkbenchReport,
};
use crate::real_data::{RealDataQuery, RealGliomaBundle};
use crate::real_data_autonomous_workflow::{
    RealDataAutonomousWorkflowQuery, RealDataAutonomousWorkflowReport,
};
use crate::real_data_cohort_landscape::{
    RealDataCohortLandscapeQuery, RealDataCohortLandscapeReport,
};
use crate::real_data_coverage::{RealDataCoverageQuery, RealDataCoverageReport};
use crate::real_data_diff::{RealDataDiffQuery, RealDataDiffReport};
use crate::real_data_draft_audit::{RealDataDraftAuditReport, RealDataDraftAuditRequest};
use crate::real_data_evidence_packet::{RealDataEvidencePacketQuery, RealDataEvidencePacketReport};
use crate::real_data_freshness::{RealDataFreshnessQuery, RealDataFreshnessReport};
use crate::real_data_molecular_coverage::RealDataMolecularCoverageQuery;
use crate::real_data_reasoning_context::{
    RealDataReasoningContextQuery, RealDataReasoningContextReport,
};
use crate::real_data_reconciliation::{RealDataReconciliationQuery, RealDataReconciliationReport};
use crate::real_data_refresh::{RealDataRefreshAuditQuery, RealDataRefreshAuditReport};
use crate::real_data_review_disposition::{
    RealDataReviewDecision, RealDataReviewDispositionReport,
};
use crate::real_data_review_queue::{RealDataReviewQueueQuery, RealDataReviewQueueReport};
use crate::real_data_trial_landscape::RealDataTrialLandscapeQuery;
use crate::research_brief::{NeurosurgicalResearchBriefQuery, NeurosurgicalResearchBriefReport};
use crate::research_plan::{
    compile as compile_research_plan, ResearchPlanReport, MAX_RESEARCH_PLAN_REFERENCES,
    MAX_RESEARCH_PLAN_TASKS,
};
use crate::specialty_evidence_map::SpecialtyEvidenceMapReport;
use crate::temporal::audit_temporal;
use crate::PublicLiteratureQuery;
use crate::{
    GliomaEvidenceState, GliomaMolecularPanel, GliomaMolecularSummary, MAX_SESSION_STEPS,
    NEUROSURGERY_SCHEMA_VERSION,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const MAX_CASE_ID_BYTES: usize = 128;
const MAX_QUESTION_BYTES: usize = 4_000;
const MAX_TEXT_BYTES: usize = 8_000;
const MAX_OBSERVATIONS: usize = 256;
const MAX_EVIDENCE: usize = 256;
const MAX_REQUESTED_TOOLS: usize = 32;

struct SessionEvidence<'a> {
    real_data: Option<&'a RealGliomaBundle>,
    real_summary: Option<&'a RealDataSummary>,
    public_literature: Option<&'a PublicLiteratureBundle>,
    public_summary: Option<&'a PublicLiteratureSummary>,
    original_request: &'a CaseRequest,
}

/// Configuration for the local agent. Bounds are intentionally explicit so a caller cannot turn
/// a read-only request into an unbounded memory or prompt construction operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NeurosurgicalAgent {
    max_observations: usize,
    max_evidence: usize,
    max_requested_tools: usize,
}

impl Default for NeurosurgicalAgent {
    fn default() -> Self {
        Self {
            max_observations: MAX_OBSERVATIONS,
            max_evidence: MAX_EVIDENCE,
            max_requested_tools: MAX_REQUESTED_TOOLS,
        }
    }
}

impl NeurosurgicalAgent {
    /// Builds a local agent with lower bounds for constrained deployments.
    pub fn bounded(
        max_observations: usize,
        max_evidence: usize,
        max_requested_tools: usize,
    ) -> Result<Self, NeurosurgeryError> {
        if max_observations == 0 || max_observations > MAX_OBSERVATIONS {
            return Err(NeurosurgeryError::TooMany {
                field: "max_observations",
                found: max_observations,
                max: MAX_OBSERVATIONS,
            });
        }
        if max_evidence == 0 || max_evidence > MAX_EVIDENCE {
            return Err(NeurosurgeryError::TooMany {
                field: "max_evidence",
                found: max_evidence,
                max: MAX_EVIDENCE,
            });
        }
        if max_requested_tools == 0 || max_requested_tools > MAX_REQUESTED_TOOLS {
            return Err(NeurosurgeryError::TooMany {
                field: "max_requested_tools",
                found: max_requested_tools,
                max: MAX_REQUESTED_TOOLS,
            });
        }
        Ok(Self {
            max_observations,
            max_evidence,
            max_requested_tools,
        })
    }

    /// Returns the closed tool catalogue without constructing an agent run.
    pub fn catalogue(&self) -> Vec<ToolSpec> {
        tool_catalogue()
    }

    /// Returns every specialty's bounded research profile for caller-side discovery.
    pub fn specialty_profiles(&self) -> Vec<SpecialtyProfile> {
        Specialty::ALL
            .iter()
            .copied()
            .map(Specialty::profile)
            .collect()
    }

    /// Route a bounded natural-language research question into the closed specialty catalogue.
    /// The lexical planner abstains on weak or ambiguous text and never infers a diagnosis,
    /// patient risk, treatment, or procedural action.
    pub fn intake_plan(
        &self,
        query: &NeurosurgicalIntakeQuery,
    ) -> Result<NeurosurgicalIntakePlan, NeurosurgeryError> {
        crate::intake::plan(query)
    }

    /// Compose a bounded research mission directly from a natural-language question.
    ///
    /// The question is used only to create an internal, research-synthesis `CaseRequest`; the
    /// returned envelope contains the intake digest and mission digest, never the request text.
    /// Glioma requires the validated real-data snapshot, while the other specialties require the
    /// validated cross-specialty PubMed snapshot. A PubMed bundle is optional supplemental
    /// citation context for a glioma mission and is never merged with population data.
    pub fn run_intake_mission(
        &self,
        query: &NeurosurgicalIntakeQuery,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        max_steps: usize,
    ) -> Result<NeurosurgicalIntakeMissionResult, NeurosurgeryError> {
        self.run_intake_mission_with_case_assets(
            query,
            real_data,
            public_literature,
            None,
            None,
            max_steps,
        )
    }

    /// Compose natural-language intake into a guarded mission while carrying an optional
    /// caller-owned, real de-identified multimodal asset manifest. The manifest is projected only
    /// after intake selects and validates a concrete case route; bytes stay outside this crate and
    /// the nested mission remains held for human review.
    #[allow(clippy::too_many_arguments)]
    pub fn run_intake_mission_with_case_assets(
        &self,
        query: &NeurosurgicalIntakeQuery,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        case_asset_manifest: Option<&CaseAssetManifest>,
        case_asset_query: Option<&CaseAssetManifestQuery>,
        max_steps: usize,
    ) -> Result<NeurosurgicalIntakeMissionResult, NeurosurgeryError> {
        self.run_intake_mission_with_case_assets_and_freshness(
            query,
            real_data,
            public_literature,
            case_asset_manifest,
            case_asset_query,
            None,
            max_steps,
        )
    }

    /// Compose natural-language intake with optional case assets and an explicit caller-clocked
    /// retrieval-age posture. Freshness is evaluated independently for each attached evidence
    /// plane and is never inferred from the host clock or bundle generation timestamp.
    #[allow(clippy::too_many_arguments)]
    pub fn run_intake_mission_with_case_assets_and_freshness(
        &self,
        query: &NeurosurgicalIntakeQuery,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        case_asset_manifest: Option<&CaseAssetManifest>,
        case_asset_query: Option<&CaseAssetManifestQuery>,
        freshness: Option<&RealDataFreshnessQuery>,
        max_steps: usize,
    ) -> Result<NeurosurgicalIntakeMissionResult, NeurosurgeryError> {
        self.run_intake_mission_with_case_inputs(
            query,
            real_data,
            public_literature,
            case_asset_manifest,
            case_asset_query,
            freshness,
            None,
            None,
            None,
            max_steps,
        )
    }

    /// Compose natural-language intake while carrying a persisted, caller-owned case-asset
    /// review ledger. The ledger is rebound to the exact manifest projection before synthesis,
    /// evidence programming, acquisition, and the final mission audit are emitted.
    #[allow(clippy::too_many_arguments)]
    pub fn run_intake_mission_with_case_assets_and_dispositions(
        &self,
        query: &NeurosurgicalIntakeQuery,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        case_asset_manifest: Option<&CaseAssetManifest>,
        case_asset_query: Option<&CaseAssetManifestQuery>,
        freshness: Option<&RealDataFreshnessQuery>,
        case_asset_disposition: Option<&CaseAssetReviewDispositionReport>,
        max_steps: usize,
    ) -> Result<NeurosurgicalIntakeMissionResult, NeurosurgeryError> {
        self.run_intake_mission_with_case_inputs(
            query,
            real_data,
            public_literature,
            case_asset_manifest,
            case_asset_query,
            freshness,
            None,
            None,
            case_asset_disposition,
            max_steps,
        )
    }

    /// Compose natural-language intake with caller-sanitized DICOM/FHIR metadata imports. The
    /// imports are projected only after lexical intake selects a concrete specialty, then routed
    /// through the same digest-bound mission helper as direct case-import runs.
    #[allow(clippy::too_many_arguments)]
    pub fn run_intake_mission_with_case_imports(
        &self,
        query: &NeurosurgicalIntakeQuery,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        dicom_import: Option<&DicomCaseImport>,
        fhir_import: Option<&FhirCaseImport>,
        freshness: Option<&RealDataFreshnessQuery>,
        max_steps: usize,
    ) -> Result<NeurosurgicalIntakeMissionResult, NeurosurgeryError> {
        self.run_intake_mission_with_case_inputs(
            query,
            real_data,
            public_literature,
            None,
            None,
            freshness,
            dicom_import,
            fhir_import,
            None,
            max_steps,
        )
    }

    /// Compose natural-language intake from sanitized DICOM/FHIR metadata while carrying a
    /// persisted reviewer ledger bound to the composed manifest projection.
    #[allow(clippy::too_many_arguments)]
    pub fn run_intake_mission_with_case_imports_and_dispositions(
        &self,
        query: &NeurosurgicalIntakeQuery,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        dicom_import: Option<&DicomCaseImport>,
        fhir_import: Option<&FhirCaseImport>,
        freshness: Option<&RealDataFreshnessQuery>,
        case_asset_disposition: Option<&CaseAssetReviewDispositionReport>,
        max_steps: usize,
    ) -> Result<NeurosurgicalIntakeMissionResult, NeurosurgeryError> {
        self.run_intake_mission_with_case_inputs(
            query,
            real_data,
            public_literature,
            None,
            None,
            freshness,
            dicom_import,
            fhir_import,
            case_asset_disposition,
            max_steps,
        )
    }

    #[allow(clippy::too_many_arguments)]
    #[inline(never)]
    fn run_intake_mission_with_case_inputs(
        &self,
        query: &NeurosurgicalIntakeQuery,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        case_asset_manifest: Option<&CaseAssetManifest>,
        case_asset_query: Option<&CaseAssetManifestQuery>,
        freshness: Option<&RealDataFreshnessQuery>,
        dicom_import: Option<&DicomCaseImport>,
        fhir_import: Option<&FhirCaseImport>,
        case_asset_disposition: Option<&CaseAssetReviewDispositionReport>,
        max_steps: usize,
    ) -> Result<NeurosurgicalIntakeMissionResult, NeurosurgeryError> {
        if (dicom_import.is_some() || fhir_import.is_some()) && case_asset_manifest.is_some() {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "DICOM/FHIR intake imports cannot be combined with a separate case asset manifest".to_string(),
            });
        }
        if case_asset_query.is_some() && case_asset_manifest.is_none() {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "a case asset manifest query requires a case asset manifest".to_string(),
            });
        }
        if case_asset_disposition.is_some()
            && case_asset_manifest.is_none()
            && dicom_import.is_none()
            && fhir_import.is_none()
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason:
                    "case-asset dispositions require a case manifest or sanitized DICOM/FHIR import"
                        .to_string(),
            });
        }
        let intake = self.intake_plan(query)?;
        let base_limitations = || {
            vec![
                "the question is transient input; only its SHA-256 digest is returned".to_string(),
                "intake creates a research-synthesis request and never authorizes clinical use"
                    .to_string(),
                "the selected mission remains read-only and terminates at human review".to_string(),
                "real and PubMed snapshots remain separate evidence planes and are never merged"
                    .to_string(),
                "an optional case-asset manifest contributes metadata-only provenance; asset bytes are never opened and every state remains reviewer-owned"
                    .to_string(),
                "bundle filters are derived only from matched closed-vocabulary terms; the raw question is never used as a returned query"
                    .to_string(),
            ]
        };
        if intake.abstained {
            return Ok(NeurosurgicalIntakeMissionResult {
                schema_version: crate::intake::NEUROSURGERY_INTAKE_MISSION_SCHEMA_VERSION
                    .to_string(),
                intake,
                status: NeurosurgicalIntakeMissionStatus::Abstained,
                request_digest: None,
                mission: None,
                required_evidence: Vec::new(),
                human_review_required: true,
                provider: "none".to_string(),
                network: false,
                effect: ToolEffect::ReadOnly,
                limitations: base_limitations(),
            });
        }

        if max_steps == 0 || max_steps > MAX_SESSION_STEPS {
            return Err(NeurosurgeryError::SessionRejected {
                reason: format!(
                    "autonomous intake mission max_steps must be between 1 and {MAX_SESSION_STEPS}"
                ),
            });
        }
        let specialty =
            intake
                .selected_specialty
                .ok_or_else(|| NeurosurgeryError::SessionRejected {
                    reason: "selected intake plan has no specialty".to_string(),
                })?;
        // Turn only the intake planner's closed-vocabulary matches into local bundle filters. The
        // raw question remains transient and is never copied into a query/report; this still lets
        // an autonomous mission narrow real records and PubMed citations to the requested topics.
        let routing_terms = intake
            .candidates
            .iter()
            .flat_map(|candidate| candidate.matched_terms.iter())
            .filter(|term| term.as_str() != "caller_explicit_specialty")
            .cloned()
            .collect::<BTreeSet<_>>();
        let routing_text = if !routing_terms.is_empty() {
            Some(routing_terms.iter().cloned().collect::<Vec<_>>().join(" "))
        } else if query.specialty.is_some() {
            // Explicit specialty hints intentionally expose only the marker
            // `caller_explicit_specialty`; keep the first source lookup useful with one
            // reviewed vocabulary term rather than sending the full question as a selector.
            Some(
                match specialty {
                    Specialty::Glioma => "glioblastoma",
                    Specialty::CranialBase => "cranial base",
                    Specialty::Craniosynostosis => "craniosynostosis",
                    Specialty::Encephalocele => "encephalocele",
                    Specialty::SpinaBifida => "spina bifida",
                    Specialty::ChiariMalformation => "chiari",
                }
                .to_string(),
            )
        } else {
            None
        };
        let real_query = RealDataQuery {
            text: routing_text.clone(),
            ..RealDataQuery::default()
        };
        let public_query = PublicLiteratureQuery {
            specialty: Some(specialty),
            text: routing_text,
            ..PublicLiteratureQuery::default()
        };
        if specialty != Specialty::Glioma && real_data.is_some() {
            return Err(NeurosurgeryError::RealDataSpecialtyUnsupported { specialty });
        }
        let required_evidence = if specialty == Specialty::Glioma {
            vec!["real_glioma_snapshot".to_string()]
        } else {
            vec!["pubmed_snapshot".to_string()]
        };
        let missing_evidence = match specialty {
            Specialty::Glioma if real_data.is_none() => required_evidence.clone(),
            _ if public_literature.is_none() && specialty != Specialty::Glioma => {
                required_evidence.clone()
            }
            _ => Vec::new(),
        };
        let mut request = if let Some(request) = query.case_request.clone() {
            if request.specialty != specialty {
                return Err(NeurosurgeryError::SessionRejected {
                    reason: format!(
                        "case_request specialty {:?} does not match selected intake specialty {:?}",
                        request.specialty, specialty
                    ),
                });
            }
            // Validate the caller-owned case before any bundle query or session step. This keeps
            // the natural-language planner a routing aid while making the composed mission useful
            // for real, de-identified observations instead of manufacturing an empty case.
            self.validate_request(&request)?;
            request
        } else {
            CaseRequest {
                schema_version: NEUROSURGERY_SCHEMA_VERSION.to_string(),
                case_id: format!("intake-{}", &intake.question_digest[..16]),
                specialty,
                request_use: RequestUse::ResearchSynthesis,
                question: query.question.clone(),
                direct_identifier_fields: Vec::new(),
                observations: Vec::new(),
                evidence: Vec::new(),
                requested_tools: Vec::new(),
                real_data_query: None,
                glioma_molecular: None,
            }
        };
        if let Some(manifest) = case_asset_manifest {
            let asset_query = case_asset_query.cloned().unwrap_or_default();
            manifest.validate_for_request(&request, &asset_query)?;
        }
        // Validate caller imports before the evidence-presence handoff. This prevents a
        // malformed or synthetic export from being smuggled through a `needs_evidence` result.
        if let Some(import) = dicom_import {
            import.project(&request)?;
        }
        if let Some(import) = fhir_import {
            import.project(&request)?;
        }
        // Validate persisted reviewer state even when the required population snapshot is
        // absent. A needs-evidence response must never become a bypass for malformed ledger
        // metadata; binding to the exact manifest report is rechecked by the mission constructor
        // once the evidence plane is available.
        if let Some(disposition) = case_asset_disposition {
            disposition.validate_integrity()?;
        }
        if !missing_evidence.is_empty() {
            return Ok(NeurosurgicalIntakeMissionResult {
                schema_version: crate::intake::NEUROSURGERY_INTAKE_MISSION_SCHEMA_VERSION
                    .to_string(),
                intake,
                status: NeurosurgicalIntakeMissionStatus::NeedsEvidence,
                request_digest: None,
                mission: None,
                required_evidence: missing_evidence,
                human_review_required: true,
                provider: "none".to_string(),
                network: false,
                effect: ToolEffect::ReadOnly,
                limitations: base_limitations(),
            });
        }
        if request
            .requested_tools
            .contains(&ToolCapability::RealDataQuery)
        {
            // Keep an explicitly requested population query executable while constraining its
            // free-text facet to the intake vocabulary. Structured facets remain caller-owned,
            // but raw intake/case text is never copied into the emitted query report.
            let mut bounded_query = request.real_data_query.take().unwrap_or_default();
            bounded_query.text = real_query.text.clone();
            request.real_data_query = Some(bounded_query);
        }
        // Several validated evidence envelopes are assembled in one route. Keep that route on a
        // bounded worker stack so callers on small application/test stacks do not overflow while
        // composing a complete real-data mission. The worker borrows all inputs and joins before
        // returning, so this remains synchronous, deterministic, and read-only.
        let mission = std::thread::scope(|scope| {
            let handle = std::thread::Builder::new()
                .name("aurora-neurosurgical-intake".to_string())
                .stack_size(8 * 1024 * 1024)
                .spawn_scoped(scope, || {
        let mission = if dicom_import.is_some() || fhir_import.is_some() {
            if case_asset_disposition.is_some() {
                self.run_research_mission_with_case_imports_and_dispositions(
                    &request,
                    real_data,
                    public_literature,
                    real_data.is_some().then_some(&real_query),
                    public_literature.is_some().then_some(&public_query),
                    freshness,
                    None,
                    dicom_import,
                    fhir_import,
                    case_asset_disposition,
                    max_steps,
                )?
            } else {
                self.run_research_mission_with_case_imports(
                    &request,
                    real_data,
                    public_literature,
                    real_data.is_some().then_some(&real_query),
                    public_literature.is_some().then_some(&public_query),
                    freshness,
                    None,
                    dicom_import,
                    fhir_import,
                    max_steps,
                )?
            }
        } else {
            match specialty {
                Specialty::Glioma => match (real_data, public_literature) {
                    (Some(real), Some(public)) => {
                        if let Some(disposition) = case_asset_disposition {
                            self.run_research_mission_with_real_data_and_public_literature_case_assets_and_dispositions(
                                &request,
                                real,
                                public,
                                Some(&real_query),
                                Some(&public_query),
                                freshness,
                                None,
                                case_asset_manifest,
                                case_asset_query,
                                Some(disposition),
                                max_steps,
                            )?
                        } else {
                            self.run_research_mission_with_real_data_and_public_literature_case_assets(
                                &request,
                                real,
                                public,
                                Some(&real_query),
                                Some(&public_query),
                                freshness,
                                None,
                                case_asset_manifest,
                                case_asset_query,
                                max_steps,
                            )?
                        }
                    }
                    (Some(real), None) => {
                        if let Some(disposition) = case_asset_disposition {
                            self.run_research_mission_with_case_assets_and_dispositions(
                                &request,
                                Some(real),
                                Some(&real_query),
                                freshness,
                                case_asset_manifest,
                                case_asset_query,
                                Some(disposition),
                                max_steps,
                            )?
                        } else {
                            self.run_research_mission_with_case_assets(
                                &request,
                                Some(real),
                                Some(&real_query),
                                freshness,
                                case_asset_manifest,
                                case_asset_query,
                                max_steps,
                            )?
                        }
                    }
                    _ => unreachable!("missing glioma evidence was handled above"),
                },
                _ => {
                    let literature =
                        public_literature.expect("missing public evidence was handled above");
                    if let Some(disposition) = case_asset_disposition {
                        self.run_research_mission_with_public_literature_case_assets_and_dispositions(
                            &request,
                            literature,
                            Some(&public_query),
                            freshness,
                            None,
                            case_asset_manifest,
                            case_asset_query,
                            Some(disposition),
                            max_steps,
                        )?
                    } else {
                        self.run_research_mission_with_public_literature_case_assets(
                            &request,
                            literature,
                            Some(&public_query),
                            freshness,
                            None,
                            case_asset_manifest,
                            case_asset_query,
                            max_steps,
                        )?
                    }
                }
            }
        };
        Ok(mission)
            })
            .map_err(|error| NeurosurgeryError::SessionRejected {
                reason: format!("could not start bounded intake worker: {error}"),
            })?;
            handle
                .join()
                .map_err(|_| NeurosurgeryError::SessionRejected {
                    reason: "bounded intake worker panicked".to_string(),
                })?
        })?;
        let request_digest = mission.run.response.request_digest.clone();
        // At this envelope level, reaching this branch means the required public snapshot was
        // present and the bounded mission completed. Any route-level observation gaps stay
        // visible in `mission.status` and `mission.evidence_gaps` for the human reviewer.
        let status = NeurosurgicalIntakeMissionStatus::ReadyForHumanReview;
        Ok(NeurosurgicalIntakeMissionResult {
            schema_version: crate::intake::NEUROSURGERY_INTAKE_MISSION_SCHEMA_VERSION.to_string(),
            intake,
            status,
            request_digest: Some(request_digest),
            mission: Some(mission),
            required_evidence: Vec::new(),
            human_review_required: true,
            provider: "none".to_string(),
            network: false,
            effect: ToolEffect::ReadOnly,
            limitations: base_limitations(),
        })
    }

    /// Compose one bounded question into a multi-specialty public-evidence portfolio.
    ///
    /// The default mode preserves intake abstention and scans only the selected specialty. An
    /// explicit `include_all_specialties` request is a corpus-reconnaissance override: it fans
    /// out the same matched vocabulary across all six lanes, keeps each lane independent, and
    /// never invents a combined clinical route. A real glioma snapshot is required whenever the
    /// selected portfolio contains glioma; the validated PubMed snapshot is always required.
    pub fn run_intake_portfolio(
        &self,
        query: &NeurosurgicalIntakePortfolioQuery,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
    ) -> Result<NeurosurgicalIntakePortfolioResult, NeurosurgeryError> {
        self.run_intake_portfolio_with_case_assets(query, real_data, public_literature, None, None)
    }

    /// Compose a bounded intake portfolio while carrying optional real, de-identified asset
    /// metadata into a single selected-lane mission. An explicit all-specialty portfolio refuses
    /// a manifest because one asset specialty cannot be attached to six independent lanes.
    #[allow(clippy::too_many_arguments)]
    pub fn run_intake_portfolio_with_case_assets(
        &self,
        query: &NeurosurgicalIntakePortfolioQuery,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        case_asset_manifest: Option<&CaseAssetManifest>,
        case_asset_query: Option<&CaseAssetManifestQuery>,
    ) -> Result<NeurosurgicalIntakePortfolioResult, NeurosurgeryError> {
        self.run_intake_portfolio_with_case_assets_and_freshness(
            query,
            real_data,
            public_literature,
            case_asset_manifest,
            case_asset_query,
            None,
        )
    }

    /// Compose an intake portfolio with optional case assets and explicit caller-clocked
    /// freshness. The freshness report remains attached to the public-literature portfolio and,
    /// for a selected lane, to its nested mission as well.
    #[allow(clippy::too_many_arguments)]
    pub fn run_intake_portfolio_with_case_assets_and_freshness(
        &self,
        query: &NeurosurgicalIntakePortfolioQuery,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        case_asset_manifest: Option<&CaseAssetManifest>,
        case_asset_query: Option<&CaseAssetManifestQuery>,
        freshness: Option<&RealDataFreshnessQuery>,
    ) -> Result<NeurosurgicalIntakePortfolioResult, NeurosurgeryError> {
        self.run_intake_portfolio_with_case_assets_and_freshness_and_dispositions(
            query,
            real_data,
            public_literature,
            case_asset_manifest,
            case_asset_query,
            freshness,
            None,
        )
    }

    /// Compose an intake portfolio while carrying a persisted, caller-owned case-asset review
    /// ledger into the selected-lane mission. Broad all-specialty portfolios still reject asset
    /// attachments so a reviewer disposition cannot be assigned to the wrong lane.
    #[allow(clippy::too_many_arguments)]
    pub fn run_intake_portfolio_with_case_assets_and_freshness_and_dispositions(
        &self,
        query: &NeurosurgicalIntakePortfolioQuery,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        case_asset_manifest: Option<&CaseAssetManifest>,
        case_asset_query: Option<&CaseAssetManifestQuery>,
        freshness: Option<&RealDataFreshnessQuery>,
        case_asset_disposition: Option<&CaseAssetReviewDispositionReport>,
    ) -> Result<NeurosurgicalIntakePortfolioResult, NeurosurgeryError> {
        if case_asset_query.is_some() && case_asset_manifest.is_none() {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "a case asset manifest query requires a case asset manifest".to_string(),
            });
        }
        if case_asset_disposition.is_some() && case_asset_manifest.is_none() {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "case-asset dispositions require a case asset manifest".to_string(),
            });
        }
        let intake = self.intake_plan(&query.intake)?;
        if query.max_session_steps == 0 || query.max_session_steps > MAX_SESSION_STEPS {
            return Err(NeurosurgeryError::SessionRejected {
                reason: format!(
                    "autonomous intake portfolio max_session_steps must be between 1 and {MAX_SESSION_STEPS}"
                ),
            });
        }
        for (field, found, max) in [
            ("max_hits_per_lane", query.max_hits_per_lane, 128),
            (
                "max_review_items_per_lane",
                query.max_review_items_per_lane,
                128,
            ),
            ("max_issues_per_lane", query.max_issues_per_lane, 256),
        ] {
            if found == 0 || found > max {
                return Err(NeurosurgeryError::TooMany { field, found, max });
            }
        }

        let selected_specialties = if query.include_all_specialties {
            Specialty::ALL.to_vec()
        } else if let Some(specialty) = intake.selected_specialty {
            vec![specialty]
        } else {
            return Ok(NeurosurgicalIntakePortfolioResult {
                schema_version: crate::intake::NEUROSURGERY_INTAKE_PORTFOLIO_SCHEMA_VERSION
                    .to_string(),
                intake,
                status: NeurosurgicalIntakeMissionStatus::Abstained,
                request_digest: None,
                mission: None,
                portfolio: None,
                selected_specialties: Vec::new(),
                required_evidence: Vec::new(),
                human_review_required: true,
                provider: "none".to_string(),
                network: false,
                effect: ToolEffect::ReadOnly,
                limitations: vec![
                    "ambiguous intake abstains unless the caller explicitly requests all six evidence lanes".to_string(),
                    "the question is transient input; only its SHA-256 digest is returned".to_string(),
                    "the portfolio is citation/workbench research and never clinical authorization".to_string(),
                ],
            });
        };

        if query.include_all_specialties && case_asset_manifest.is_some() {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "a case asset manifest may attach only to a single selected intake lane; all-specialty portfolios keep asset provenance caller-owned"
                    .to_string(),
            });
        }
        if let Some(disposition) = case_asset_disposition {
            disposition.validate_integrity()?;
        }

        if real_data.is_some() && !selected_specialties.contains(&Specialty::Glioma) {
            return Err(NeurosurgeryError::RealDataSpecialtyUnsupported {
                specialty: selected_specialties[0],
            });
        }
        let mut required_evidence = Vec::new();
        if public_literature.is_none() {
            required_evidence.push("pubmed_snapshot".to_string());
        }
        if selected_specialties.contains(&Specialty::Glioma) && real_data.is_none() {
            required_evidence.push("real_glioma_snapshot".to_string());
        }
        if !required_evidence.is_empty() {
            return Ok(NeurosurgicalIntakePortfolioResult {
                schema_version: crate::intake::NEUROSURGERY_INTAKE_PORTFOLIO_SCHEMA_VERSION
                    .to_string(),
                intake,
                status: NeurosurgicalIntakeMissionStatus::NeedsEvidence,
                request_digest: None,
                mission: None,
                portfolio: None,
                selected_specialties,
                required_evidence,
                human_review_required: true,
                provider: "none".to_string(),
                network: false,
                effect: ToolEffect::ReadOnly,
                limitations: vec![
                    "the required validated snapshots must be caller-supplied; no source is fetched automatically".to_string(),
                    "the question is transient input; only its SHA-256 digest is returned".to_string(),
                    "the portfolio remains read-only and requires human review".to_string(),
                ],
            });
        }

        let routing_terms = intake
            .candidates
            .iter()
            .flat_map(|candidate| candidate.matched_terms.iter())
            .filter(|term| term.as_str() != "caller_explicit_specialty")
            .cloned()
            .collect::<BTreeSet<_>>();
        let routing_text = if !routing_terms.is_empty() {
            Some(routing_terms.iter().cloned().collect::<Vec<_>>().join(" "))
        } else if query.intake.specialty.is_some() {
            let specialty = selected_specialties[0];
            Some(
                match specialty {
                    Specialty::Glioma => "glioblastoma",
                    Specialty::CranialBase => "cranial base",
                    Specialty::Craniosynostosis => "craniosynostosis",
                    Specialty::Encephalocele => "encephalocele",
                    Specialty::SpinaBifida => "spina bifida",
                    Specialty::ChiariMalformation => "chiari",
                }
                .to_string(),
            )
        } else {
            None
        };
        let portfolio = public_literature
            .expect("missing public literature was handled above")
            .literature_portfolio(&PublicLiteraturePortfolioQuery {
                specialties: Some(selected_specialties.clone()),
                text: routing_text,
                max_hits_per_lane: query.max_hits_per_lane,
                max_review_items_per_lane: query.max_review_items_per_lane,
                max_issues_per_lane: query.max_issues_per_lane,
                freshness: freshness.cloned(),
                ..PublicLiteraturePortfolioQuery::default()
            })?;

        let mission_result =
            if !query.include_all_specialties && intake.selected_specialty.is_some() {
                Some(self.run_intake_mission_with_case_inputs(
                    &query.intake,
                    real_data,
                    public_literature,
                    case_asset_manifest,
                    case_asset_query,
                    freshness,
                    None,
                    None,
                    case_asset_disposition,
                    query.max_session_steps,
                )?)
            } else {
                None
            };
        let request_digest = mission_result
            .as_ref()
            .and_then(|result| result.request_digest.clone());
        let mission = mission_result.and_then(|result| result.mission);
        Ok(NeurosurgicalIntakePortfolioResult {
            schema_version: crate::intake::NEUROSURGERY_INTAKE_PORTFOLIO_SCHEMA_VERSION.to_string(),
            intake,
            status: NeurosurgicalIntakeMissionStatus::ReadyForHumanReview,
            request_digest,
            mission,
            portfolio: Some(portfolio),
            selected_specialties,
            required_evidence: Vec::new(),
            human_review_required: true,
            provider: "none".to_string(),
            network: false,
            effect: ToolEffect::ReadOnly,
            limitations: vec![
                "each specialty lane is an independent citation/workbench handoff; no cohorts or biology are merged".to_string(),
                "only matched closed-vocabulary terms become local filters; the raw question is never returned".to_string(),
                "empty, truncated, and unverified metadata remain explicit reviewer work".to_string(),
                "the portfolio never ranks specialties, recommends care, or authorizes diagnosis, treatment, triage, or procedure".to_string(),
            ],
        })
    }

    /// Audit granular specialty intake coverage without executing the route or inferring a
    /// diagnosis. The returned digest binds the exact caller request, while each row preserves
    /// measured, unmeasured, uninterpretable, conflicting, and provenance-gap states.
    pub fn audit_evidence(
        &self,
        request: &CaseRequest,
    ) -> Result<EvidenceAuditReport, NeurosurgeryError> {
        self.validate_request(request)?;
        audit_evidence(request)
    }

    /// Project caller-registered, real de-identified multimodal assets into a digest-only review
    /// manifest. The core never opens asset bytes, follows paths, or interprets clinical content.
    pub fn case_asset_manifest(
        &self,
        request: &CaseRequest,
        manifest: &CaseAssetManifest,
        query: &CaseAssetManifestQuery,
    ) -> Result<CaseAssetManifestReport, NeurosurgeryError> {
        self.validate_request(request)?;
        manifest.project(request, query)
    }

    /// Import a caller-sanitized real FHIR Bundle as digest-only asset metadata. The importer
    /// never opens references or interprets clinical content; unclassified resources remain
    /// explicit reviewer obligations.
    pub fn case_fhir_import(
        &self,
        request: &CaseRequest,
        import: &FhirCaseImport,
    ) -> Result<FhirCaseImportReport, NeurosurgeryError> {
        self.validate_request(request)?;
        import.project(request)
    }

    /// Import caller-supplied DICOM JSON series metadata into a digest-only imaging inventory.
    /// Pixel data, private tags, patient identifiers, and clinical interpretation never cross the
    /// boundary; missing modality, anatomy, dates, or object digests remain review tasks.
    pub fn case_dicom_import(
        &self,
        request: &CaseRequest,
        import: &DicomCaseImport,
    ) -> Result<DicomCaseImportReport, NeurosurgeryError> {
        self.validate_request(request)?;
        import.project(request)
    }

    /// Project a de-identified DICOM JSON export and immediately bind its digest-only asset
    /// report into the source-grounded synthesis, evidence-program, and acquisition workers.
    /// Pixel bytes and clinical interpretation never cross this boundary; the returned
    /// acquisition checkpoint is caller-persisted and held for human review.
    pub fn case_dicom_evidence_workflow(
        &self,
        request: &CaseRequest,
        import: &DicomCaseImport,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        query: &DicomEvidenceWorkflowQuery,
    ) -> Result<DicomEvidenceWorkflowReport, NeurosurgeryError> {
        self.validate_request(request)?;
        crate::case_dicom_workflow::run(request, import, real_data, public_literature, query)
    }

    /// Apply caller-owned reviewer dispositions to a previously projected case-asset report.
    /// The report digest is revalidated before any sequence is accepted; this never changes the
    /// manifest or opens the referenced asset bytes.
    pub fn case_asset_review_disposition(
        &self,
        report: &CaseAssetManifestReport,
        decisions: &[crate::CaseAssetReviewDecision],
    ) -> Result<crate::CaseAssetReviewDispositionReport, NeurosurgeryError> {
        report.apply_review_dispositions(decisions)
    }

    /// Audit explicit observation dates and de-identified timepoint labels without inferring a
    /// trajectory. Missing dates, same-time records, and caller-order inversions remain visible.
    pub fn temporal_audit(
        &self,
        request: &CaseRequest,
    ) -> Result<crate::TemporalAlignmentReport, NeurosurgeryError> {
        self.validate_request(request)?;
        audit_temporal(request)
    }

    /// Compile a bounded, source-linked research handoff from the intake audit. Optional bundles
    /// are validated exactly as they are for a route, but their records remain population/citation
    /// context and never become patient observations. No source is fetched and no clinical action
    /// is represented.
    pub fn plan_research(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        max_tasks: usize,
        max_references_per_task: usize,
    ) -> Result<ResearchPlanReport, NeurosurgeryError> {
        if real_data.is_some() && public_literature.is_some() {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "research planning accepts one evidence bundle: choose real glioma data or public literature".to_string(),
            });
        }
        self.validate_request(request)?;
        if let Some(data) = real_data {
            // Reuse the route's real-data boundary so synthetic markers, specialty drift, and
            // requests for population-only tools cannot be bypassed by the planner.
            self.prepare_request(request, Some(data))?;
        } else if let Some(literature) = public_literature {
            self.prepare_public_literature_request(request, literature)?;
        } else {
            self.prepare_request(request, None)?;
        }
        compile_research_plan(
            request,
            real_data,
            public_literature,
            max_tasks,
            max_references_per_task,
        )
    }

    /// Compile a bounded autonomous acquisition wave across the caller-supplied real glioma and
    /// public-literature planes. Every emitted step is a deterministic local query and remains
    /// held for qualified human review; no source is fetched and no asset bytes are opened.
    pub fn evidence_acquisition(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        query: &crate::EvidenceAcquisitionQuery,
    ) -> Result<crate::EvidenceAcquisitionReport, NeurosurgeryError> {
        self.validate_request(request)?;
        crate::evidence_acquisition::compile(request, real_data, public_literature, query)
    }

    /// Compile a bounded acquisition wave while carrying a digest-only case-asset review
    /// projection. This keeps the source-query worker and multimodal metadata obligations in one
    /// restart-safe plan without opening or interpreting any asset bytes.
    pub fn evidence_acquisition_with_case_assets(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        case_asset_report: Option<&CaseAssetManifestReport>,
        query: &crate::EvidenceAcquisitionQuery,
    ) -> Result<crate::EvidenceAcquisitionReport, NeurosurgeryError> {
        self.validate_request(request)?;
        crate::evidence_acquisition::compile_with_case_assets(
            request,
            real_data,
            public_literature,
            case_asset_report,
            query,
        )
    }

    /// Compile an acquisition wave bound to a persisted case-asset review ledger.
    #[allow(clippy::too_many_arguments)]
    pub fn evidence_acquisition_with_case_assets_and_dispositions(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        case_asset_report: Option<&CaseAssetManifestReport>,
        dispositions: &CaseAssetReviewDispositionReport,
        query: &crate::EvidenceAcquisitionQuery,
    ) -> Result<crate::EvidenceAcquisitionReport, NeurosurgeryError> {
        self.validate_request(request)?;
        crate::evidence_acquisition::compile_with_case_assets_and_dispositions(
            request,
            real_data,
            public_literature,
            case_asset_report,
            dispositions,
            query,
        )
    }

    /// Start a digest-bound, caller-persisted acquisition worker over validated public snapshots.
    pub fn evidence_acquisition_start(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        query: &crate::EvidenceAcquisitionQuery,
    ) -> Result<crate::EvidenceAcquisitionStartResult, NeurosurgeryError> {
        self.validate_request(request)?;
        crate::evidence_acquisition::start(request, real_data, public_literature, query)
    }

    /// Start a digest-bound acquisition worker with an optional case-asset review projection.
    pub fn evidence_acquisition_start_with_case_assets(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        case_asset_report: Option<&CaseAssetManifestReport>,
        query: &crate::EvidenceAcquisitionQuery,
    ) -> Result<crate::EvidenceAcquisitionStartResult, NeurosurgeryError> {
        self.validate_request(request)?;
        crate::evidence_acquisition::start_with_case_assets(
            request,
            real_data,
            public_literature,
            case_asset_report,
            query,
        )
    }

    /// Start an acquisition worker with a persisted case-asset review ledger in its plan and
    /// checkpoint envelope.
    #[allow(clippy::too_many_arguments)]
    pub fn evidence_acquisition_start_with_case_assets_and_dispositions(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        case_asset_report: Option<&CaseAssetManifestReport>,
        dispositions: &CaseAssetReviewDispositionReport,
        query: &crate::EvidenceAcquisitionQuery,
    ) -> Result<crate::EvidenceAcquisitionStartResult, NeurosurgeryError> {
        self.validate_request(request)?;
        crate::evidence_acquisition::start_with_case_assets_and_dispositions(
            request,
            real_data,
            public_literature,
            case_asset_report,
            dispositions,
            query,
        )
    }

    /// Advance a caller-owned acquisition checkpoint by a bounded number of local replay steps.
    pub fn evidence_acquisition_advance(
        &self,
        session: &crate::EvidenceAcquisitionSession,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        query: &crate::EvidenceAcquisitionQuery,
        max_steps: usize,
    ) -> Result<crate::EvidenceAcquisitionAdvanceResult, NeurosurgeryError> {
        self.validate_request(request)?;
        crate::evidence_acquisition::advance(
            session,
            request,
            real_data,
            public_literature,
            query,
            max_steps,
        )
    }

    /// Advance a caller-owned acquisition checkpoint while re-binding the same case-asset
    /// projection used at start.
    #[allow(clippy::too_many_arguments)]
    pub fn evidence_acquisition_advance_with_case_assets(
        &self,
        session: &crate::EvidenceAcquisitionSession,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        case_asset_report: Option<&CaseAssetManifestReport>,
        query: &crate::EvidenceAcquisitionQuery,
        max_steps: usize,
    ) -> Result<crate::EvidenceAcquisitionAdvanceResult, NeurosurgeryError> {
        self.validate_request(request)?;
        crate::evidence_acquisition::advance_with_case_assets(
            session,
            request,
            real_data,
            public_literature,
            case_asset_report,
            query,
            max_steps,
        )
    }

    /// Advance a disposition-bound acquisition checkpoint while re-validating the same ledger.
    #[allow(clippy::too_many_arguments)]
    pub fn evidence_acquisition_advance_with_case_assets_and_dispositions(
        &self,
        session: &crate::EvidenceAcquisitionSession,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        case_asset_report: Option<&CaseAssetManifestReport>,
        dispositions: &CaseAssetReviewDispositionReport,
        query: &crate::EvidenceAcquisitionQuery,
        max_steps: usize,
    ) -> Result<crate::EvidenceAcquisitionAdvanceResult, NeurosurgeryError> {
        self.validate_request(request)?;
        crate::evidence_acquisition::advance_with_case_assets_and_dispositions(
            session,
            request,
            real_data,
            public_literature,
            case_asset_report,
            dispositions,
            query,
            max_steps,
        )
    }

    /// Finish an acquisition worker only after every step and required source plane are present.
    pub fn evidence_acquisition_finish(
        &self,
        session: &crate::EvidenceAcquisitionSession,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        query: &crate::EvidenceAcquisitionQuery,
    ) -> Result<crate::EvidenceAcquisitionExecutionReport, NeurosurgeryError> {
        self.validate_request(request)?;
        crate::evidence_acquisition::finish(session, request, real_data, public_literature, query)
    }

    /// Finish a caller-owned acquisition checkpoint while preserving its case-asset binding.
    pub fn evidence_acquisition_finish_with_case_assets(
        &self,
        session: &crate::EvidenceAcquisitionSession,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        case_asset_report: Option<&CaseAssetManifestReport>,
        query: &crate::EvidenceAcquisitionQuery,
    ) -> Result<crate::EvidenceAcquisitionExecutionReport, NeurosurgeryError> {
        self.validate_request(request)?;
        crate::evidence_acquisition::finish_with_case_assets(
            session,
            request,
            real_data,
            public_literature,
            case_asset_report,
            query,
        )
    }

    /// Finish a disposition-bound acquisition checkpoint after every step has been replayed.
    #[allow(clippy::too_many_arguments)]
    pub fn evidence_acquisition_finish_with_case_assets_and_dispositions(
        &self,
        session: &crate::EvidenceAcquisitionSession,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        case_asset_report: Option<&CaseAssetManifestReport>,
        dispositions: &CaseAssetReviewDispositionReport,
        query: &crate::EvidenceAcquisitionQuery,
    ) -> Result<crate::EvidenceAcquisitionExecutionReport, NeurosurgeryError> {
        self.validate_request(request)?;
        crate::evidence_acquisition::finish_with_case_assets_and_dispositions(
            session,
            request,
            real_data,
            public_literature,
            case_asset_report,
            dispositions,
            query,
        )
    }

    /// Project explicit source crosswalks from a validated public glioma bundle. This is a
    /// metadata graph for caller-owned traversal, not a biological or clinical inference graph.
    pub fn evidence_graph(
        &self,
        real_data: &RealGliomaBundle,
        query: &EvidenceGraphQuery,
    ) -> Result<EvidenceGraphReport, NeurosurgeryError> {
        real_data.evidence_graph(query)
    }

    /// Project validated public-glioma coverage, temporal axes, and linkage gaps without scoring
    /// evidence or interpreting it as a clinical conclusion.
    pub fn real_data_coverage(
        &self,
        real_data: &RealGliomaBundle,
        query: &RealDataCoverageQuery,
    ) -> Result<RealDataCoverageReport, NeurosurgeryError> {
        real_data.coverage_report(query)
    }

    /// Compare aggregate public genomic projects, released-case inventory, and file-type
    /// metadata without opening files or treating projects as interchangeable cohorts.
    pub fn real_data_cohort_landscape(
        &self,
        real_data: &RealGliomaBundle,
        query: &RealDataCohortLandscapeQuery,
    ) -> Result<RealDataCohortLandscapeReport, NeurosurgeryError> {
        real_data.cohort_landscape(query)
    }

    /// Reconcile exact PMID/DOI crosswalks inside one validated public snapshot. Findings are
    /// metadata review obligations only; no record is repaired, merged, ranked, or interpreted.
    pub fn real_data_reconciliation(
        &self,
        real_data: &RealGliomaBundle,
        query: &RealDataReconciliationQuery,
    ) -> Result<RealDataReconciliationReport, NeurosurgeryError> {
        real_data.reconcile(query)
    }

    /// Audit the age of every source in a validated real-glioma snapshot against an explicit
    /// caller-supplied clock. This is a freshness posture, never an evidence-quality or clinical
    /// relevance judgment.
    pub fn real_data_freshness(
        &self,
        real_data: &RealGliomaBundle,
        query: &RealDataFreshnessQuery,
    ) -> Result<RealDataFreshnessReport, NeurosurgeryError> {
        real_data.freshness_report(query)
    }

    /// Compare two validated public snapshots without fetching or interpreting their records.
    pub fn real_data_diff(
        &self,
        before: &RealGliomaBundle,
        after: &RealGliomaBundle,
        query: &RealDataDiffQuery,
    ) -> Result<RealDataDiffReport, NeurosurgeryError> {
        before.diff(after, query)
    }

    /// Reconcile two validated real-data snapshots into one digest-bound refresh review. The
    /// candidate is never merged or accepted automatically; all structural changes, freshness
    /// states, metadata obligations, and brief unknowns remain visible to the caller.
    pub fn real_data_refresh_audit(
        &self,
        request: &CaseRequest,
        before: &RealGliomaBundle,
        after: &RealGliomaBundle,
        query: &RealDataRefreshAuditQuery,
    ) -> Result<RealDataRefreshAuditReport, NeurosurgeryError> {
        self.prepare_request(request, Some(after))?;
        before.refresh_audit(after, query, request)
    }

    /// Derive structural metadata-review obligations from one validated public snapshot.
    pub fn real_data_review_queue(
        &self,
        real_data: &RealGliomaBundle,
        query: &RealDataReviewQueueQuery,
    ) -> Result<RealDataReviewQueueReport, NeurosurgeryError> {
        real_data.review_queue(query)
    }

    /// Apply caller-owned, digest-bound dispositions to emitted real-data review tasks.
    pub fn real_data_review_disposition(
        &self,
        queue: &RealDataReviewQueueReport,
        decisions: &[RealDataReviewDecision],
    ) -> Result<RealDataReviewDispositionReport, NeurosurgeryError> {
        queue.apply_dispositions(decisions)
    }

    /// Compose a bounded, source-linked evidence packet for a local model or human reviewer.
    pub fn real_data_evidence_packet(
        &self,
        real_data: &RealGliomaBundle,
        query: &RealDataEvidencePacketQuery,
    ) -> Result<RealDataEvidencePacketReport, NeurosurgeryError> {
        real_data.evidence_packet(query)
    }

    /// Compose one deterministic, resumable review wave from the validated real-data packet.
    /// Actions are metadata obligations only; persisted human dispositions can close or reopen
    /// them without changing the source snapshot.
    pub fn real_data_autonomous_workflow(
        &self,
        real_data: &RealGliomaBundle,
        query: &RealDataAutonomousWorkflowQuery,
    ) -> Result<RealDataAutonomousWorkflowReport, NeurosurgeryError> {
        real_data.autonomous_workflow(query)
    }

    /// Render a bounded, digest-bound real-data packet for caller-owned local-model context.
    /// Source text remains untrusted data and every included record is returned as a citation.
    pub fn real_data_reasoning_context(
        &self,
        real_data: &RealGliomaBundle,
        query: &RealDataReasoningContextQuery,
    ) -> Result<RealDataReasoningContextReport, NeurosurgeryError> {
        real_data.reasoning_context(query)
    }

    /// Audit a local-model or reviewer draft against one freshly composed real-data packet.
    /// Grounding is structural only; claim text is never interpreted as a clinical conclusion.
    pub fn real_data_draft_audit(
        &self,
        real_data: &RealGliomaBundle,
        request: &RealDataDraftAuditRequest,
    ) -> Result<RealDataDraftAuditReport, NeurosurgeryError> {
        real_data.audit_draft(request)
    }

    /// Extract a bounded, source-linked research brief from exactly one validated public bundle.
    /// Topic membership is deterministic lexical extraction; no model, network, patient file, or
    /// clinical interpretation is involved.
    pub fn research_brief(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        query: &NeurosurgicalResearchBriefQuery,
    ) -> Result<NeurosurgicalResearchBriefReport, NeurosurgeryError> {
        if real_data.is_some() && public_literature.is_some() {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "research brief accepts one evidence bundle: choose real glioma data or public literature".to_string(),
            });
        }
        self.validate_request(request)?;
        match (real_data, public_literature) {
            (Some(data), None) => {
                self.prepare_request(request, Some(data))?;
                data.research_brief(request, query)
            }
            (None, Some(literature)) => {
                self.prepare_public_literature_request(request, literature)?;
                literature.research_brief(request, query)
            }
            (None, None) => Err(NeurosurgeryError::RealDataRejected {
                reason: "research brief requires a validated real-data or public-literature bundle"
                    .to_string(),
            }),
            (Some(_), Some(_)) => Err(NeurosurgeryError::RealDataRejected {
                reason: "research brief accepts one evidence bundle: choose real glioma data or public literature".to_string(),
            }),
        }
    }

    /// Align a de-identified case with one or both validated public evidence planes. The
    /// resulting ledger is source-addressable and digest-bound, but deliberately contains no
    /// generated diagnosis, prognosis, treatment, triage, or procedural conclusion.
    pub fn evidence_synthesis(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        query: &EvidenceSynthesisQuery,
    ) -> Result<EvidenceSynthesisReport, NeurosurgeryError> {
        self.validate_request(request)?;
        crate::evidence_synthesis::synthesize(request, real_data, public_literature, query)
    }

    /// Align the same evidence planes while binding a previously projected real multimodal
    /// asset report into the synthesis digest. Asset bytes remain outside the crate and are never
    /// opened or interpreted.
    pub fn evidence_synthesis_with_case_assets(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        query: &EvidenceSynthesisQuery,
        case_asset_report: Option<&CaseAssetManifestReport>,
    ) -> Result<EvidenceSynthesisReport, NeurosurgeryError> {
        self.validate_request(request)?;
        crate::evidence_synthesis::synthesize_with_case_assets(
            request,
            real_data,
            public_literature,
            query,
            case_asset_report,
        )
    }

    /// Align the same evidence planes while binding a case-asset projection and a validated,
    /// reviewer-owned disposition ledger. The disposition ledger must be digest-bound to the
    /// exact projection; no asset bytes or clinical interpretation enter this path.
    pub fn evidence_synthesis_with_case_assets_and_dispositions(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        query: &EvidenceSynthesisQuery,
        case_asset_report: Option<&CaseAssetManifestReport>,
        disposition_report: Option<&CaseAssetReviewDispositionReport>,
    ) -> Result<EvidenceSynthesisReport, NeurosurgeryError> {
        self.validate_request(request)?;
        crate::evidence_synthesis::synthesize_with_case_assets_and_dispositions(
            request,
            real_data,
            public_literature,
            query,
            case_asset_report,
            disposition_report,
        )
    }

    /// Ground each typed glioma molecular marker against exact records in caller-supplied public
    /// snapshots. A hit is retrieval metadata only; the map never interprets a marker clinically.
    pub fn glioma_molecular_map(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        query: &GliomaMolecularMapQuery,
    ) -> Result<GliomaMolecularEvidenceMapReport, NeurosurgeryError> {
        self.validate_request(request)?;
        crate::glioma_molecular_map::map_molecular_evidence(
            request,
            real_data,
            public_literature,
            query,
        )
    }

    /// Build the domain-specific evidence coverage map for one validated request. The map keeps
    /// each specialty's identity, spatial, functional, and temporal dimensions explicit without
    /// interpreting the caller's observation values.
    pub fn specialty_evidence_map(
        &self,
        request: &CaseRequest,
    ) -> Result<SpecialtyEvidenceMapReport, NeurosurgeryError> {
        self.validate_request(request)?;
        crate::specialty_evidence_map::build_specialty_evidence_map(request)
    }

    /// Compose a bounded public-literature packet for any supported neurosurgical specialty.
    pub fn public_literature_evidence_packet(
        &self,
        public_literature: &PublicLiteratureBundle,
        query: &PublicLiteratureEvidencePacketQuery,
    ) -> Result<PublicLiteratureEvidencePacketReport, NeurosurgeryError> {
        public_literature.evidence_packet(query)
    }

    /// Render a bounded, digest-bound public-literature packet for caller-owned local-model
    /// context. Source text remains untrusted data and every included PMID is addressable.
    pub fn public_literature_reasoning_context(
        &self,
        public_literature: &PublicLiteratureBundle,
        query: &PublicLiteratureReasoningContextQuery,
    ) -> Result<PublicLiteratureReasoningContextReport, NeurosurgeryError> {
        public_literature.reasoning_context(query)
    }

    /// Audit a cross-specialty local-model or reviewer draft against emitted PMID citations.
    pub fn public_literature_draft_audit(
        &self,
        public_literature: &PublicLiteratureBundle,
        request: &PublicLiteratureDraftAuditRequest,
    ) -> Result<PublicLiteratureDraftAuditReport, NeurosurgeryError> {
        public_literature.audit_draft(request)
    }

    /// Fan out a bounded real-literature query across one or more specialty lanes while
    /// preserving each lane's source-linked packet and explicit empty/truncation state.
    pub fn public_literature_matrix(
        &self,
        public_literature: &PublicLiteratureBundle,
        query: &PublicLiteratureMatrixQuery,
    ) -> Result<PublicLiteratureMatrixReport, NeurosurgeryError> {
        public_literature.literature_matrix(query)
    }

    /// Audit the age of every source in a validated cross-specialty PubMed snapshot against an
    /// explicit caller-supplied clock. Future-dated metadata remains a review state.
    pub fn public_literature_freshness(
        &self,
        public_literature: &PublicLiteratureBundle,
        query: &RealDataFreshnessQuery,
    ) -> Result<RealDataFreshnessReport, NeurosurgeryError> {
        public_literature.freshness_report(query)
    }

    /// Reconcile two validated cross-specialty PubMed snapshots without fetching or promoting
    /// the candidate. The report keeps source/PMID identity, lane coverage, and freshness facts
    /// separate from human review obligations.
    pub fn public_literature_refresh_audit(
        &self,
        before: &PublicLiteratureBundle,
        after: &PublicLiteratureBundle,
        query: &PublicLiteratureRefreshAuditQuery,
    ) -> Result<PublicLiteratureRefreshAuditReport, NeurosurgeryError> {
        before.refresh_audit(after, query)
    }

    /// Link the real glioma literature index to the selected lane of the validated cross-specialty
    /// PubMed snapshot by exact PMID/DOI identifiers only. The report never merges either bundle.
    pub fn literature_link_audit(
        &self,
        real_data: &RealGliomaBundle,
        public_literature: &PublicLiteratureBundle,
        query: &LiteratureLinkAuditQuery,
    ) -> Result<LiteratureLinkAuditReport, NeurosurgeryError> {
        real_data.literature_link_audit(public_literature, query)
    }

    /// Audit source/record completeness and identifier hygiene in a validated public snapshot.
    pub fn public_literature_integrity_audit(
        &self,
        public_literature: &PublicLiteratureBundle,
        query: &PublicLiteratureIntegrityAuditQuery,
    ) -> Result<PublicLiteratureIntegrityAuditReport, NeurosurgeryError> {
        public_literature.integrity_audit(query)
    }

    /// Derive stable reviewer-owned tasks from a validated public-literature integrity audit.
    /// Missing fields and duplicate identifiers remain explicit; no source is fetched or edited.
    pub fn public_literature_review_queue(
        &self,
        public_literature: &PublicLiteratureBundle,
        query: &PublicLiteratureReviewQueueQuery,
    ) -> Result<PublicLiteratureReviewQueueReport, NeurosurgeryError> {
        public_literature.review_queue(query)
    }

    /// Join a specialty's explicit research profile to real PubMed coverage and metadata gaps.
    /// The workbench is a reviewer navigation surface, never a readiness or clinical score.
    pub fn public_literature_workbench(
        &self,
        public_literature: &PublicLiteratureBundle,
        query: &PublicLiteratureWorkbenchQuery,
    ) -> Result<PublicLiteratureWorkbenchReport, NeurosurgeryError> {
        public_literature.specialty_workbench(query)
    }

    /// Build a bounded, source-grounded agenda across the selected specialty lanes. Every track
    /// is projected from exact records in the supplied snapshots; lexical matches are never
    /// promoted to diagnosis, prognosis, treatment, or operative guidance.
    pub fn evidence_program(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        query: &EvidenceProgramQuery,
    ) -> Result<EvidenceProgramReport, NeurosurgeryError> {
        self.validate_request(request)?;
        crate::build_evidence_program(request, real_data, public_literature, query)
    }

    /// Build the evidence program and join it to a validated, digest-only case-asset projection.
    /// The raw manifest is validated and projected first; asset bytes never enter this crate.
    pub fn evidence_program_with_case_assets(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        manifest: &CaseAssetManifest,
        manifest_query: &CaseAssetManifestQuery,
        query: &EvidenceProgramQuery,
    ) -> Result<EvidenceProgramReport, NeurosurgeryError> {
        let asset_report = self.case_asset_manifest(request, manifest, manifest_query)?;
        self.evidence_program_for_asset_report(
            request,
            real_data,
            public_literature,
            Some(&asset_report),
            query,
        )
    }

    /// Build the evidence program with both the digest-only case-asset projection and its
    /// persisted review-disposition ledger. The ledger is bound by manifest digest and counts,
    /// so a stale or mismatched review state fails closed before it can affect a worklist.
    #[allow(clippy::too_many_arguments)]
    pub fn evidence_program_with_case_assets_and_dispositions(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        manifest: &CaseAssetManifest,
        manifest_query: &CaseAssetManifestQuery,
        dispositions: &CaseAssetReviewDispositionReport,
        query: &EvidenceProgramQuery,
    ) -> Result<EvidenceProgramReport, NeurosurgeryError> {
        let asset_report = self.case_asset_manifest(request, manifest, manifest_query)?;
        self.evidence_program_for_asset_report_and_dispositions(
            request,
            real_data,
            public_literature,
            Some(&asset_report),
            dispositions,
            query,
        )
    }

    fn evidence_program_for_asset_report(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        asset_report: Option<&CaseAssetManifestReport>,
        query: &EvidenceProgramQuery,
    ) -> Result<EvidenceProgramReport, NeurosurgeryError> {
        self.validate_request(request)?;
        crate::build_evidence_program_with_asset_report(
            request,
            real_data,
            public_literature,
            asset_report,
            query,
        )
    }

    pub(crate) fn evidence_program_for_asset_report_and_dispositions(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        asset_report: Option<&CaseAssetManifestReport>,
        dispositions: &CaseAssetReviewDispositionReport,
        query: &EvidenceProgramQuery,
    ) -> Result<EvidenceProgramReport, NeurosurgeryError> {
        self.validate_request(request)?;
        crate::build_evidence_program_with_asset_report_and_dispositions(
            request,
            real_data,
            public_literature,
            asset_report,
            dispositions,
            query,
        )
    }

    /// Run a bounded, source-linked metadata pass across the selected public-literature lanes.
    /// The portfolio is autonomous orchestration for research review, never a clinical score.
    pub fn public_literature_portfolio(
        &self,
        public_literature: &PublicLiteratureBundle,
        query: &PublicLiteraturePortfolioQuery,
    ) -> Result<PublicLiteraturePortfolioReport, NeurosurgeryError> {
        public_literature.literature_portfolio(query)
    }

    /// Runs the complete read-only route and returns a reproducible report.
    pub fn run(&self, request: &CaseRequest) -> Result<AgentResponse, NeurosurgeryError> {
        self.run_internal(request, false)
    }

    fn run_internal(
        &self,
        request: &CaseRequest,
        real_data_attached: bool,
    ) -> Result<AgentResponse, NeurosurgeryError> {
        self.validate_request(request)?;
        if !real_data_attached
            && request.requested_tools.iter().any(|tool| {
                matches!(
                    tool,
                    ToolCapability::RealDataInventory | ToolCapability::RealDataQuery
                )
            })
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "real_data_inventory requires a validated public glioma bundle".to_string(),
            });
        }
        let request_digest = digest(request)?;
        let required = required_capabilities(request.specialty);
        let route = self.route(&required, &request.requested_tools)?;
        let evidence_gaps = evidence_gaps(request, &route);
        let plan = route
            .iter()
            .enumerate()
            .map(|(index, capability)| PlanStep {
                ordinal: (index + 1) as u16,
                capability: *capability,
                purpose: tool_spec(*capability).purpose,
                effect: ToolEffect::ReadOnly,
                requires_human_review: true,
            })
            .collect::<Vec<_>>();
        let tool_runs = route
            .iter()
            .map(|capability| run_tool(*capability, request, &evidence_gaps))
            .collect::<Vec<_>>();
        let hypotheses = research_hypotheses(request.specialty);
        let glioma_molecular = request
            .glioma_molecular
            .as_ref()
            .map(GliomaMolecularPanel::summary)
            .transpose()?;
        let report = build_report(request, &evidence_gaps, glioma_molecular.as_ref());
        let temporal_alignment = Some(audit_temporal(request)?);
        let specialty_evidence_map = Some(
            crate::specialty_evidence_map::build_specialty_evidence_map(request)?,
        );
        let status = if evidence_gaps.is_empty() {
            AgentStatus::ReadyForHumanReview
        } else {
            AgentStatus::NeedsEvidence
        };

        let mut response = AgentResponse {
            schema_version: NEUROSURGERY_SCHEMA_VERSION.to_string(),
            response_digest: String::new(),
            request_digest,
            specialty: request.specialty,
            specialty_profile: request.specialty.profile(),
            status,
            plan,
            tool_runs,
            evidence_gaps,
            hypotheses,
            report,
            real_data: None,
            public_literature: None,
            temporal_alignment,
            glioma_molecular,
            specialty_evidence_map,
        };
        seal_response(&mut response)?;
        Ok(response)
    }

    /// Runs a glioma research request with a validated, public-data snapshot.
    ///
    /// The bundle is treated as population-level evidence (registry, genomic-project and study
    /// metadata), never as an individual patient's record. Its guideline references are added to
    /// the evidence inventory and its digest is exposed in the response so a downstream system
    /// can reproduce exactly which real-data snapshot was used. No network or API key is needed.
    pub fn run_with_real_glioma_data(
        &self,
        request: &CaseRequest,
        data: &RealGliomaBundle,
    ) -> Result<AgentResponse, NeurosurgeryError> {
        let (enriched, summary) = self.prepare_request(request, Some(data))?;
        let summary = summary.expect("real-data preparation returns a summary");
        let mut response = self.run_internal(&enriched, true)?;
        response.real_data = Some(summary.clone());
        response.report.known_inputs.push(format!(
            "validated real-data bundle: {} source(s), {} record(s), digest {}",
            summary.source_count, summary.record_count, summary.bundle_digest
        ));
        response.report.uncertainties.push(
            "real-data records are population-level research evidence and do not represent this case's patient-level measurements".to_string(),
        );
        for run in &mut response.tool_runs {
            annotate_real_data_tool_run(run, &summary, data, &enriched)?;
        }
        seal_response(&mut response)?;
        Ok(response)
    }

    /// Run any specialty route with a validated cross-specialty PubMed snapshot.
    ///
    /// The snapshot is projected into unverified, source-labelled evidence records and the
    /// ordinary route is then executed. This deliberately does not enable the glioma registry or
    /// cBioPortal tools: those tools require `RealGliomaBundle`, while this lane provides citation
    /// metadata for cranial-base, craniofacial, encephalocele, spina-bifida, Chiari, and glioma
    /// review alike.
    pub fn run_with_public_literature(
        &self,
        request: &CaseRequest,
        literature: &PublicLiteratureBundle,
    ) -> Result<AgentResponse, NeurosurgeryError> {
        let (enriched, summary) = self.prepare_public_literature_request(request, literature)?;
        let mut response = self.run_internal(&enriched, false)?;
        response.public_literature = Some(summary.clone());
        response.report.known_inputs.push(format!(
            "validated public literature: {} source(s), {} citation record(s), {} specialty lane(s), digest {}",
            summary.source_count,
            summary.record_count,
            summary.specialty_counts.len(),
            summary.bundle_digest
        ));
        response.report.uncertainties.push(
            "PubMed abstracts and indexing tags are unverified citation metadata; they do not establish study quality, applicability, cohort identity, or a patient-level finding".to_string(),
        );
        for run in &mut response.tool_runs {
            annotate_public_literature_tool_run(run, &summary, literature);
        }
        seal_response(&mut response)?;
        Ok(response)
    }

    /// Prepare a request with only the caller's specialty lane from a validated public bundle.
    /// Keeping this transformation in one helper means one-shot runs and resumable sessions bind
    /// the exact same evidence bytes and cannot silently diverge.
    fn prepare_public_literature_request(
        &self,
        request: &CaseRequest,
        literature: &PublicLiteratureBundle,
    ) -> Result<(CaseRequest, PublicLiteratureSummary), NeurosurgeryError> {
        self.validate_request(request)?;
        if request.request_use == RequestUse::SyntheticCaseSimulation {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "synthetic_case_simulation cannot be combined with public literature"
                    .to_string(),
            });
        }
        if request.requested_tools.iter().any(|tool| {
            matches!(
                tool,
                ToolCapability::RealDataInventory | ToolCapability::RealDataQuery
            )
        }) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "the cross-specialty literature bundle supports citation synthesis only; use RealGliomaBundle for registry/profile tools".to_string(),
            });
        }
        if !literature.has_specialty(request.specialty) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: format!(
                    "public-literature bundle has no records tagged for {}",
                    request.specialty.slug()
                ),
            });
        }
        let summary = literature.summary()?;
        let mut enriched = request.clone();
        let records = literature.evidence_records_for_specialty(Some(request.specialty));
        let combined = enriched.evidence.len().saturating_add(records.len());
        if combined > self.max_evidence {
            return Err(NeurosurgeryError::TooMany {
                field: "evidence_with_public_literature",
                found: combined,
                max: self.max_evidence,
            });
        }
        enriched.evidence.extend(records);
        Ok((enriched, summary))
    }

    /// Start a caller-persisted, tool-by-tool research session. The returned state is complete
    /// enough to checkpoint as JSON; no server-side session memory is created.
    pub fn start_session(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
    ) -> Result<NeurosurgicalSession, NeurosurgeryError> {
        let (prepared, summary) = self.prepare_request(request, real_data)?;
        self.start_session_prepared(
            &prepared,
            summary.map(|summary| summary.bundle_digest),
            None,
        )
    }

    /// Start a caller-persisted session backed by the validated cross-specialty PubMed bundle.
    pub fn start_session_with_public_literature(
        &self,
        request: &CaseRequest,
        literature: &PublicLiteratureBundle,
    ) -> Result<NeurosurgicalSession, NeurosurgeryError> {
        let (prepared, summary) = self.prepare_public_literature_request(request, literature)?;
        self.start_session_prepared(&prepared, None, Some(summary.bundle_digest))
    }

    fn start_session_prepared(
        &self,
        prepared: &CaseRequest,
        real_data_digest: Option<String>,
        public_literature_digest: Option<String>,
    ) -> Result<NeurosurgicalSession, NeurosurgeryError> {
        let request_digest = digest(prepared)?;
        let route = self.route(
            &required_capabilities(prepared.specialty),
            &prepared.requested_tools,
        )?;
        let session_id = format!("ns-session-{}", &request_digest[..16]);
        let event_chain_digest =
            digest_value(&(session_id.as_str(), request_digest.as_str(), &route))?;
        Ok(NeurosurgicalSession {
            schema_version: NEUROSURGERY_SCHEMA_VERSION.to_string(),
            session_id,
            request_digest,
            real_data_digest,
            public_literature_digest,
            specialty: prepared.specialty,
            route,
            next_ordinal: 1,
            status: SessionStatus::Planned,
            event_chain_digest,
            events: Vec::new(),
        })
    }

    /// Advance one read-only tool and return the new state. Callers may persist the returned
    /// value after every step and resume it later with the same request and data bundle.
    pub fn advance_session(
        &self,
        session: &NeurosurgicalSession,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
    ) -> Result<NeurosurgicalSession, NeurosurgeryError> {
        self.validate_session(session)?;
        let (prepared, summary) = self.prepare_request(request, real_data)?;
        self.advance_session_prepared(
            session,
            &prepared,
            SessionEvidence {
                real_data,
                real_summary: summary.as_ref(),
                public_literature: None,
                public_summary: None,
                original_request: request,
            },
        )
    }

    /// Advance one route step for a session backed by a validated public-literature bundle.
    pub fn advance_session_with_public_literature(
        &self,
        session: &NeurosurgicalSession,
        request: &CaseRequest,
        literature: &PublicLiteratureBundle,
    ) -> Result<NeurosurgicalSession, NeurosurgeryError> {
        self.validate_session(session)?;
        let (prepared, summary) = self.prepare_public_literature_request(request, literature)?;
        self.advance_session_prepared(
            session,
            &prepared,
            SessionEvidence {
                real_data: None,
                real_summary: None,
                public_literature: Some(literature),
                public_summary: Some(&summary),
                original_request: request,
            },
        )
    }

    fn advance_session_prepared(
        &self,
        session: &NeurosurgicalSession,
        prepared: &CaseRequest,
        evidence: SessionEvidence<'_>,
    ) -> Result<NeurosurgicalSession, NeurosurgeryError> {
        self.assert_session_inputs(
            session,
            prepared,
            evidence.real_summary,
            evidence.public_summary,
        )?;
        if session.next_ordinal as usize > session.route.len() {
            return Err(NeurosurgeryError::SessionRejected {
                reason: "session has no remaining route step".to_string(),
            });
        }
        let capability = session.route[session.next_ordinal as usize - 1];
        let gaps = evidence_gaps(prepared, &session.route);
        let mut tool_run = run_tool(capability, prepared, &gaps);
        if let (Some(summary), Some(data)) = (evidence.real_summary, evidence.real_data) {
            annotate_real_data_tool_run(&mut tool_run, summary, data, prepared)?;
        }
        if let (Some(summary), Some(data)) = (evidence.public_summary, evidence.public_literature) {
            annotate_public_literature_tool_run(&mut tool_run, summary, data);
        }
        let finding_digest = digest_value(&tool_run)?;
        let event_digest = session_event_digest(
            session.next_ordinal,
            capability,
            tool_run.status,
            &finding_digest,
            &session.event_chain_digest,
        )?;
        let mut next = session.clone();
        next.events.push(SessionEvent {
            ordinal: session.next_ordinal,
            capability,
            status: tool_run.status,
            finding_digest,
            previous_event_digest: session.event_chain_digest.clone(),
            event_digest: event_digest.clone(),
        });
        next.event_chain_digest = event_digest;
        next.next_ordinal = next.next_ordinal.saturating_add(1);
        next.status = if capability == ToolCapability::HumanReviewHold {
            SessionStatus::AwaitingHumanReview
        } else if tool_run.status == ToolRunStatus::NeedsInput {
            SessionStatus::NeedsInput
        } else {
            SessionStatus::Running
        };
        Ok(next)
    }

    /// Execute a complete checkpointed route in one bounded call and return its terminal state.
    ///
    /// This is the local autonomous loop: it uses exactly the same `start_session`,
    /// `advance_session`, and `finish_session` boundaries exposed to remote callers. The loop is
    /// intentionally finite and returns the terminal checkpoint alongside the report so a caller
    /// can persist the event chain or resume auditing without relying on server-side memory.
    pub fn run_session_to_review(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        max_steps: usize,
    ) -> Result<NeurosurgicalRunResult, NeurosurgeryError> {
        if max_steps == 0 || max_steps > MAX_SESSION_STEPS {
            return Err(NeurosurgeryError::SessionRejected {
                reason: format!(
                    "autonomous session max_steps must be between 1 and {MAX_SESSION_STEPS}"
                ),
            });
        }
        let mut session = self.start_session(request, real_data)?;
        let mut steps_executed = 0usize;
        while session.next_ordinal as usize <= session.route.len() {
            if steps_executed == max_steps {
                return Err(NeurosurgeryError::SessionRejected {
                    reason: format!(
                        "autonomous session exceeded max_steps ({max_steps}) before human-review hold"
                    ),
                });
            }
            session = self.advance_session(&session, request, real_data)?;
            steps_executed += 1;
        }
        let response = self.finish_session(&session, request, real_data)?;
        Ok(NeurosurgicalRunResult {
            schema_version: NEUROSURGERY_SCHEMA_VERSION.to_string(),
            steps_executed,
            session,
            response,
        })
    }

    /// Execute a bounded checkpointed route against the validated cross-specialty literature
    /// bundle and return the terminal checkpoint plus report.
    pub fn run_session_to_review_with_public_literature(
        &self,
        request: &CaseRequest,
        literature: &PublicLiteratureBundle,
        max_steps: usize,
    ) -> Result<NeurosurgicalRunResult, NeurosurgeryError> {
        if max_steps == 0 || max_steps > MAX_SESSION_STEPS {
            return Err(NeurosurgeryError::SessionRejected {
                reason: format!(
                    "autonomous session max_steps must be between 1 and {MAX_SESSION_STEPS}"
                ),
            });
        }
        let mut session = self.start_session_with_public_literature(request, literature)?;
        let mut steps_executed = 0usize;
        while session.next_ordinal as usize <= session.route.len() {
            if steps_executed == max_steps {
                return Err(NeurosurgeryError::SessionRejected {
                    reason: format!(
                        "autonomous session exceeded max_steps ({max_steps}) before human-review hold"
                    ),
                });
            }
            session = self.advance_session_with_public_literature(&session, request, literature)?;
            steps_executed += 1;
        }
        let response = self.finish_session_with_public_literature(&session, request, literature)?;
        Ok(NeurosurgicalRunResult {
            schema_version: NEUROSURGERY_SCHEMA_VERSION.to_string(),
            steps_executed,
            session,
            response,
        })
    }

    /// Compose catalogue discovery, an optional public-bundle query, and the bounded session
    /// worker into one provider-free mission. This is the core implementation used by MCP and
    /// the local CLI, so adapters cannot drift into a less guarded orchestration path.
    pub fn run_research_mission(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        query: Option<&RealDataQuery>,
        max_steps: usize,
    ) -> Result<NeurosurgicalMissionResult, NeurosurgeryError> {
        self.run_research_mission_with_freshness(request, real_data, query, None, max_steps)
    }

    /// Compose the mission with an optional explicit caller-clocked freshness posture. The
    /// freshness query is never inferred from the host clock or bundle generation time.
    pub fn run_research_mission_with_freshness(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        query: Option<&RealDataQuery>,
        freshness: Option<&RealDataFreshnessQuery>,
        max_steps: usize,
    ) -> Result<NeurosurgicalMissionResult, NeurosurgeryError> {
        self.run_research_mission_with_case_assets(
            request, real_data, query, freshness, None, None, max_steps,
        )
    }

    /// Compose a real-data mission while attaching an optional de-identified multimodal asset
    /// manifest. The manifest is projected before the session runs and its report is carried in
    /// the mission envelope; asset bytes remain outside this crate and the session still ends at
    /// the human-review hold.
    #[allow(clippy::too_many_arguments)]
    pub fn run_research_mission_with_case_assets(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        query: Option<&RealDataQuery>,
        freshness: Option<&RealDataFreshnessQuery>,
        case_asset_manifest: Option<&CaseAssetManifest>,
        case_asset_query: Option<&CaseAssetManifestQuery>,
        max_steps: usize,
    ) -> Result<NeurosurgicalMissionResult, NeurosurgeryError> {
        if case_asset_query.is_some() && case_asset_manifest.is_none() {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "a case asset manifest query requires a case asset manifest".to_string(),
            });
        }
        let case_asset_manifest = case_asset_manifest
            .map(|manifest| {
                let query = case_asset_query.cloned().unwrap_or_default();
                self.case_asset_manifest(request, manifest, &query)
            })
            .transpose()?;
        if request.specialty == Specialty::Glioma && real_data.is_none() {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "glioma research missions require a validated real-data bundle".to_string(),
            });
        }
        if request.specialty != Specialty::Glioma && real_data.is_none() {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "non-glioma research missions require a validated public-literature bundle"
                    .to_string(),
            });
        }
        let query_result = match query {
            Some(query) => {
                let data = real_data.ok_or_else(|| NeurosurgeryError::RealDataRejected {
                    reason: "a public-data mission query requires a real-data bundle".to_string(),
                })?;
                Some(data.query(query)?)
            }
            None => None,
        };
        let real_data_coverage = real_data
            .map(|data| data.coverage_report(&RealDataCoverageQuery::default()))
            .transpose()?;
        let real_data_trial_landscape = real_data
            .map(|data| data.trial_landscape(&RealDataTrialLandscapeQuery::default()))
            .transpose()?;
        let real_data_molecular_coverage = real_data
            .map(|data| data.molecular_coverage(&RealDataMolecularCoverageQuery::default()))
            .transpose()?;
        let real_data_cohort_landscape = real_data
            .map(|data| data.cohort_landscape(&RealDataCohortLandscapeQuery::default()))
            .transpose()?;
        let real_data_review_queue = real_data
            .map(|data| data.review_queue(&RealDataReviewQueueQuery::default()))
            .transpose()?;
        if freshness.is_some() && real_data.is_none() {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "a real-data mission freshness query requires a real-data bundle"
                    .to_string(),
            });
        }
        let real_data_freshness = match (real_data, freshness) {
            (Some(data), Some(query)) => Some(data.freshness_report(query)?),
            _ => None,
        };
        let real_data_evidence_graph = real_data
            .map(|data| data.evidence_graph(&EvidenceGraphQuery::default()))
            .transpose()?;
        let real_data_reasoning_context = real_data
            .map(|data| {
                let context_query = RealDataReasoningContextQuery {
                    packet: RealDataEvidencePacketQuery {
                        query: query.cloned().unwrap_or_default(),
                        freshness: freshness.cloned(),
                        ..RealDataEvidencePacketQuery::default()
                    },
                    ..RealDataReasoningContextQuery::default()
                };
                data.reasoning_context(&context_query)
            })
            .transpose()?;
        let real_data_evidence_packet = real_data
            .map(|data| {
                let packet_query = RealDataEvidencePacketQuery {
                    query: query.cloned().unwrap_or_default(),
                    freshness: freshness.cloned(),
                    ..RealDataEvidencePacketQuery::default()
                };
                data.evidence_packet(&packet_query)
            })
            .transpose()?;
        let real_data_autonomous_workflow = real_data
            .map(|data| {
                let workflow_query = RealDataAutonomousWorkflowQuery {
                    packet: RealDataEvidencePacketQuery {
                        query: query.cloned().unwrap_or_default(),
                        freshness: freshness.cloned(),
                        ..RealDataEvidencePacketQuery::default()
                    },
                    ..RealDataAutonomousWorkflowQuery::default()
                };
                data.autonomous_workflow(&workflow_query)
            })
            .transpose()?;
        let research_plan = Some(self.plan_research(
            request,
            real_data,
            None,
            MAX_RESEARCH_PLAN_TASKS,
            MAX_RESEARCH_PLAN_REFERENCES,
        )?);
        let evidence_program = Some(self.evidence_program_for_asset_report(
            request,
            real_data,
            None,
            case_asset_manifest.as_ref(),
            &EvidenceProgramQuery {
                freshness: freshness.cloned(),
                ..EvidenceProgramQuery::default()
            },
        )?);
        let acquisition_start = self.evidence_acquisition_start_with_case_assets(
            request,
            real_data,
            None,
            case_asset_manifest.as_ref(),
            &crate::EvidenceAcquisitionQuery {
                freshness: freshness.cloned(),
                ..crate::EvidenceAcquisitionQuery::default()
            },
        )?;
        let evidence_acquisition = Some(acquisition_start.plan);
        let evidence_acquisition_session = Some(acquisition_start.session);
        let research_brief = real_data
            .map(|data| {
                let query = NeurosurgicalResearchBriefQuery {
                    real_data_query: query.cloned(),
                    freshness: freshness.cloned(),
                    ..NeurosurgicalResearchBriefQuery::default()
                };
                data.research_brief(request, &query)
            })
            .transpose()?;
        let evidence_synthesis = Some(self.evidence_synthesis_with_case_assets(
            request,
            real_data,
            None,
            &EvidenceSynthesisQuery {
                real_data_query: query.cloned(),
                freshness: freshness.cloned(),
                ..EvidenceSynthesisQuery::default()
            },
            case_asset_manifest.as_ref(),
        )?);
        let run = self.run_session_to_review(request, real_data, max_steps)?;
        let mission_id = format!(
            "neurosurgical-mission-{}",
            &run.response.request_digest[..16]
        );
        let mut mission = NeurosurgicalMissionResult {
            schema: "bioprism-neurosurgical-research-mission/0.1".to_string(),
            mission_id,
            specialty: request.specialty,
            status: run.response.status,
            human_review_required: true,
            provider: "none".to_string(),
            network: false,
            effects: vec![ToolEffect::ReadOnly],
            catalogue: MissionCatalogue {
                specialty_count: self.specialty_profiles().len(),
                tool_count: self.catalogue().len(),
            },
            case_asset_manifest,
            case_dicom_import: None,
            case_fhir_import: None,
            case_asset_review_disposition: None,
            real_data_query: query_result,
            public_literature_query: None,
            real_data_coverage,
            real_data_trial_landscape,
            real_data_molecular_coverage,
            real_data_cohort_landscape,
            // Keep the mission-level map bound to the caller's original request. The terminal
            // session response may carry a prepared request digest after public-bundle metadata
            // enrichment; mission audits intentionally bind this projection to the original
            // case envelope instead.
            specialty_evidence_map: Some(self.specialty_evidence_map(request)?),
            real_data_review_queue,
            real_data_evidence_packet,
            real_data_autonomous_workflow,
            real_data_freshness,
            real_data_evidence_graph,
            real_data_reasoning_context,
            public_literature_reasoning_context: None,
            public_literature_evidence_packet: None,
            public_literature_freshness: None,
            public_literature_integrity_audit: None,
            public_literature_review_queue: None,
            public_literature_workbench: None,
            public_literature_portfolio: None,
            literature_link_audit: None,
            evidence_synthesis,
            research_plan,
            evidence_program,
            evidence_acquisition,
            evidence_acquisition_session,
            research_brief,
            mission_audit: None,
            run,
        };
        mission.mission_audit = Some(crate::audit_mission(&mission, request, real_data, None)?);
        Ok(mission)
    }

    /// Compose a glioma mission directly from a caller-supplied DICOM JSON export and the
    /// validated real-data snapshot. The DICOM importer is the sole source of the case-asset
    /// projection; synthesis, evidence programming, and acquisition are rebound to that exact
    /// report before the mission audit is sealed. Pixel bytes and clinical interpretation never
    /// enter the mission envelope.
    pub fn run_research_mission_with_case_dicom(
        &self,
        request: &CaseRequest,
        real_data: &RealGliomaBundle,
        query: Option<&RealDataQuery>,
        freshness: Option<&RealDataFreshnessQuery>,
        dicom_import: &DicomCaseImport,
        max_steps: usize,
    ) -> Result<NeurosurgicalMissionResult, NeurosurgeryError> {
        let dicom_report = self.case_dicom_import(request, dicom_import)?;
        let mut mission = self.run_research_mission_with_case_assets(
            request,
            Some(real_data),
            query,
            freshness,
            None,
            None,
            max_steps,
        )?;
        let asset_report = dicom_report.manifest_report.clone();
        mission.case_asset_manifest = Some(asset_report.clone());
        mission.case_dicom_import = Some(dicom_report);
        mission.evidence_synthesis = Some(self.evidence_synthesis_with_case_assets(
            request,
            Some(real_data),
            None,
            &EvidenceSynthesisQuery {
                real_data_query: query.cloned(),
                freshness: freshness.cloned(),
                ..EvidenceSynthesisQuery::default()
            },
            Some(&asset_report),
        )?);
        mission.evidence_program = Some(self.evidence_program_for_asset_report(
            request,
            Some(real_data),
            None,
            Some(&asset_report),
            &EvidenceProgramQuery {
                freshness: freshness.cloned(),
                ..EvidenceProgramQuery::default()
            },
        )?);
        let acquisition_start = self.evidence_acquisition_start_with_case_assets(
            request,
            Some(real_data),
            None,
            Some(&asset_report),
            &crate::EvidenceAcquisitionQuery {
                freshness: freshness.cloned(),
                ..crate::EvidenceAcquisitionQuery::default()
            },
        )?;
        mission.evidence_acquisition = Some(acquisition_start.plan);
        mission.evidence_acquisition_session = Some(acquisition_start.session);
        mission.mission_audit = Some(crate::audit_mission(
            &mission,
            request,
            Some(real_data),
            None,
        )?);
        Ok(mission)
    }

    /// Compose a mission from a caller-sanitized FHIR Bundle and its validated evidence bundle.
    /// The FHIR importer is the sole source of the case-asset projection; resource payloads,
    /// references, identifiers, and clinical values never enter the mission envelope.
    #[allow(clippy::too_many_arguments)]
    pub fn run_research_mission_with_case_fhir(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        query: Option<&RealDataQuery>,
        public_query: Option<&PublicLiteratureQuery>,
        freshness: Option<&RealDataFreshnessQuery>,
        portfolio_query: Option<&PublicLiteraturePortfolioQuery>,
        fhir_import: &FhirCaseImport,
        max_steps: usize,
    ) -> Result<NeurosurgicalMissionResult, NeurosurgeryError> {
        if let (Some(real_data), Some(public_literature)) = (real_data, public_literature) {
            let mut mission = self
                .run_research_mission_with_real_data_and_public_literature_case_assets_and_dispositions(
                    request,
                    real_data,
                    public_literature,
                    query,
                    public_query,
                    freshness,
                    portfolio_query,
                    None,
                    None,
                    None,
                    max_steps,
                )?;
            self.attach_fhir_report_to_mission(
                &mut mission,
                request,
                Some(real_data),
                Some(public_literature),
                query,
                public_query,
                freshness,
                fhir_import,
            )?;
            return Ok(mission);
        }

        let mut mission = if let Some(real_data) = real_data {
            self.run_research_mission_with_case_assets(
                request,
                Some(real_data),
                query,
                freshness,
                None,
                None,
                max_steps,
            )?
        } else if let Some(public_literature) = public_literature {
            self.run_research_mission_with_public_literature_case_assets_and_dispositions(
                request,
                public_literature,
                public_query,
                freshness,
                portfolio_query,
                None,
                None,
                None,
                max_steps,
            )?
        } else {
            return Err(NeurosurgeryError::RealDataRejected {
                reason:
                    "FHIR-backed missions require a validated real-data or public-literature bundle"
                        .to_string(),
            });
        };
        self.attach_fhir_report_to_mission(
            &mut mission,
            request,
            real_data,
            public_literature,
            query,
            public_query,
            freshness,
            fhir_import,
        )?;
        Ok(mission)
    }

    #[allow(clippy::too_many_arguments)]
    fn attach_fhir_report_to_mission(
        &self,
        mission: &mut NeurosurgicalMissionResult,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        query: Option<&RealDataQuery>,
        public_query: Option<&PublicLiteratureQuery>,
        freshness: Option<&RealDataFreshnessQuery>,
        fhir_import: &FhirCaseImport,
    ) -> Result<(), NeurosurgeryError> {
        let fhir_report = self.case_fhir_import(request, fhir_import)?;
        let asset_report = fhir_report.manifest_report.clone();
        mission.case_asset_manifest = Some(asset_report.clone());
        mission.case_fhir_import = Some(fhir_report);
        mission.evidence_synthesis = Some(self.evidence_synthesis_with_case_assets(
            request,
            real_data,
            public_literature,
            &EvidenceSynthesisQuery {
                real_data_query: query.cloned(),
                public_literature_query: public_query.cloned(),
                freshness: freshness.cloned(),
                ..EvidenceSynthesisQuery::default()
            },
            Some(&asset_report),
        )?);
        mission.evidence_program = Some(self.evidence_program_for_asset_report(
            request,
            real_data,
            public_literature,
            Some(&asset_report),
            &EvidenceProgramQuery {
                freshness: freshness.cloned(),
                ..EvidenceProgramQuery::default()
            },
        )?);
        let acquisition_start = self.evidence_acquisition_start_with_case_assets(
            request,
            real_data,
            public_literature,
            Some(&asset_report),
            &crate::EvidenceAcquisitionQuery {
                freshness: freshness.cloned(),
                ..crate::EvidenceAcquisitionQuery::default()
            },
        )?;
        mission.evidence_acquisition = Some(acquisition_start.plan);
        mission.evidence_acquisition_session = Some(acquisition_start.session);
        mission.mission_audit = Some(crate::audit_mission(
            mission,
            request,
            real_data,
            public_literature,
        )?);
        Ok(())
    }

    /// Compose one mission from any combination of sanitized DICOM and FHIR metadata exports.
    /// Each importer remains independent and digest-only; when both are present their validated
    /// projections are unioned into one case-asset manifest without recovering local identifiers.
    /// Population and literature planes remain separate, and the route still ends at human review.
    #[allow(clippy::too_many_arguments)]
    pub fn run_research_mission_with_case_imports(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        query: Option<&RealDataQuery>,
        public_query: Option<&PublicLiteratureQuery>,
        freshness: Option<&RealDataFreshnessQuery>,
        portfolio_query: Option<&PublicLiteraturePortfolioQuery>,
        dicom_import: Option<&DicomCaseImport>,
        fhir_import: Option<&FhirCaseImport>,
        max_steps: usize,
    ) -> Result<NeurosurgicalMissionResult, NeurosurgeryError> {
        if dicom_import.is_none() && fhir_import.is_none() {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "case-import mission requires a DICOM or FHIR metadata export".to_string(),
            });
        }
        let mut mission = match (real_data, public_literature) {
            (Some(real_data), Some(public_literature)) => self
                .run_research_mission_with_real_data_and_public_literature_case_assets_and_dispositions(
                    request,
                    real_data,
                    public_literature,
                    query,
                    public_query,
                    freshness,
                    portfolio_query,
                    None,
                    None,
                    None,
                    max_steps,
                )?,
            (Some(real_data), None) => self.run_research_mission_with_case_assets(
                request,
                Some(real_data),
                query,
                freshness,
                None,
                None,
                max_steps,
            )?,
            (None, Some(public_literature)) => self
                .run_research_mission_with_public_literature_case_assets_and_dispositions(
                    request,
                    public_literature,
                    public_query,
                    freshness,
                    portfolio_query,
                    None,
                    None,
                    None,
                    max_steps,
                )?,
            (None, None) => {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason:
                        "case-import missions require a validated real-data or public-literature bundle"
                            .to_string(),
                })
            }
        };
        let dicom_report = dicom_import
            .map(|import| self.case_dicom_import(request, import))
            .transpose()?;
        let fhir_report = fhir_import
            .map(|import| self.case_fhir_import(request, import))
            .transpose()?;
        let asset_report = match (dicom_report.as_ref(), fhir_report.as_ref()) {
            (Some(dicom), Some(fhir)) => crate::CaseAssetManifestReport::compose_for_request(
                request,
                &[&dicom.manifest_report, &fhir.manifest_report],
            )?,
            (Some(dicom), None) => dicom.manifest_report.clone(),
            (None, Some(fhir)) => fhir.manifest_report.clone(),
            (None, None) => unreachable!("case-import presence was checked above"),
        };
        mission.case_asset_manifest = Some(asset_report.clone());
        mission.case_dicom_import = dicom_report;
        mission.case_fhir_import = fhir_report;
        mission.evidence_synthesis = Some(self.evidence_synthesis_with_case_assets(
            request,
            real_data,
            public_literature,
            &EvidenceSynthesisQuery {
                real_data_query: query.cloned(),
                public_literature_query: public_query.cloned(),
                freshness: freshness.cloned(),
                ..EvidenceSynthesisQuery::default()
            },
            Some(&asset_report),
        )?);
        mission.evidence_program = Some(self.evidence_program_for_asset_report(
            request,
            real_data,
            public_literature,
            Some(&asset_report),
            &EvidenceProgramQuery {
                freshness: freshness.cloned(),
                ..EvidenceProgramQuery::default()
            },
        )?);
        let acquisition_start = self.evidence_acquisition_start_with_case_assets(
            request,
            real_data,
            public_literature,
            Some(&asset_report),
            &crate::EvidenceAcquisitionQuery {
                freshness: freshness.cloned(),
                ..crate::EvidenceAcquisitionQuery::default()
            },
        )?;
        mission.evidence_acquisition = Some(acquisition_start.plan);
        mission.evidence_acquisition_session = Some(acquisition_start.session);
        mission.mission_audit = Some(crate::audit_mission(
            &mission,
            request,
            real_data,
            public_literature,
        )?);
        Ok(mission)
    }

    /// Rebuild a DICOM/FHIR-backed mission with a persisted case-asset disposition ledger. The
    /// importer projections are first composed exactly as in the canonical constructor, then
    /// every downstream reviewer worker is rebound to that same manifest and ledger.
    #[allow(clippy::too_many_arguments)]
    pub fn run_research_mission_with_case_imports_and_dispositions(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        query: Option<&RealDataQuery>,
        public_query: Option<&PublicLiteratureQuery>,
        freshness: Option<&RealDataFreshnessQuery>,
        portfolio_query: Option<&PublicLiteraturePortfolioQuery>,
        dicom_import: Option<&DicomCaseImport>,
        fhir_import: Option<&FhirCaseImport>,
        case_asset_disposition: Option<&CaseAssetReviewDispositionReport>,
        max_steps: usize,
    ) -> Result<NeurosurgicalMissionResult, NeurosurgeryError> {
        let mut mission = self.run_research_mission_with_case_imports(
            request,
            real_data,
            public_literature,
            query,
            public_query,
            freshness,
            portfolio_query,
            dicom_import,
            fhir_import,
            max_steps,
        )?;
        let Some(disposition) = case_asset_disposition else {
            return Ok(mission);
        };
        let synthesis_query = EvidenceSynthesisQuery {
            real_data_query: query.cloned(),
            public_literature_query: public_query.cloned(),
            freshness: freshness.cloned(),
            ..EvidenceSynthesisQuery::default()
        };
        mission.evidence_synthesis =
            Some(self.evidence_synthesis_with_case_assets_and_dispositions(
                request,
                real_data,
                public_literature,
                &synthesis_query,
                mission.case_asset_manifest.as_ref(),
                Some(disposition),
            )?);
        mission.evidence_program = Some(self.evidence_program_for_asset_report_and_dispositions(
            request,
            real_data,
            public_literature,
            mission.case_asset_manifest.as_ref(),
            disposition,
            &EvidenceProgramQuery {
                freshness: freshness.cloned(),
                ..EvidenceProgramQuery::default()
            },
        )?);
        let acquisition_start = self.evidence_acquisition_start_with_case_assets_and_dispositions(
            request,
            real_data,
            public_literature,
            mission.case_asset_manifest.as_ref(),
            disposition,
            &crate::EvidenceAcquisitionQuery {
                freshness: freshness.cloned(),
                ..crate::EvidenceAcquisitionQuery::default()
            },
        )?;
        mission.evidence_acquisition = Some(acquisition_start.plan);
        mission.evidence_acquisition_session = Some(acquisition_start.session);
        mission.case_asset_review_disposition = Some(disposition.clone());
        mission.mission_audit = Some(crate::audit_mission(
            &mission,
            request,
            real_data,
            public_literature,
        )?);
        Ok(mission)
    }

    /// Rebuild a real-data mission with an optional persisted case-asset disposition ledger.
    /// The existing mission route remains the canonical constructor; this additive wrapper
    /// rebinds synthesis, evidence programming, acquisition, and the final audit after
    /// validating reviewer state against the emitted manifest projection.
    #[allow(clippy::too_many_arguments)]
    pub fn run_research_mission_with_case_assets_and_dispositions(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        query: Option<&RealDataQuery>,
        freshness: Option<&RealDataFreshnessQuery>,
        case_asset_manifest: Option<&CaseAssetManifest>,
        case_asset_query: Option<&CaseAssetManifestQuery>,
        case_asset_disposition: Option<&CaseAssetReviewDispositionReport>,
        max_steps: usize,
    ) -> Result<NeurosurgicalMissionResult, NeurosurgeryError> {
        let mut mission = self.run_research_mission_with_case_assets(
            request,
            real_data,
            query,
            freshness,
            case_asset_manifest,
            case_asset_query,
            max_steps,
        )?;
        if let Some(disposition) = case_asset_disposition {
            let synthesis_query = EvidenceSynthesisQuery {
                real_data_query: query.cloned(),
                freshness: freshness.cloned(),
                ..EvidenceSynthesisQuery::default()
            };
            mission.evidence_synthesis =
                Some(self.evidence_synthesis_with_case_assets_and_dispositions(
                    request,
                    real_data,
                    None,
                    &synthesis_query,
                    mission.case_asset_manifest.as_ref(),
                    Some(disposition),
                )?);
            mission.evidence_program =
                Some(self.evidence_program_for_asset_report_and_dispositions(
                    request,
                    real_data,
                    None,
                    mission.case_asset_manifest.as_ref(),
                    disposition,
                    &EvidenceProgramQuery {
                        freshness: freshness.cloned(),
                        ..EvidenceProgramQuery::default()
                    },
                )?);
            let acquisition_start = self
                .evidence_acquisition_start_with_case_assets_and_dispositions(
                    request,
                    real_data,
                    None,
                    mission.case_asset_manifest.as_ref(),
                    disposition,
                    &crate::EvidenceAcquisitionQuery {
                        freshness: freshness.cloned(),
                        ..crate::EvidenceAcquisitionQuery::default()
                    },
                )?;
            mission.evidence_acquisition = Some(acquisition_start.plan);
            mission.evidence_acquisition_session = Some(acquisition_start.session);
            mission.case_asset_review_disposition = Some(disposition.clone());
            mission.mission_audit = Some(crate::audit_mission(&mission, request, real_data, None)?);
        }
        Ok(mission)
    }

    /// Compose the catalogue, an optional bounded PubMed query, and a resumable public-literature
    /// session for any supported specialty. The bundle remains caller-supplied and source-bound;
    /// this helper never fetches or interprets records.
    pub fn run_research_mission_with_public_literature(
        &self,
        request: &CaseRequest,
        literature: &PublicLiteratureBundle,
        query: Option<&PublicLiteratureQuery>,
        max_steps: usize,
    ) -> Result<NeurosurgicalMissionResult, NeurosurgeryError> {
        self.run_research_mission_with_public_literature_freshness(
            request, literature, query, None, max_steps,
        )
    }

    /// Compose a public-literature mission with an optional explicit caller-clocked freshness
    /// posture. The source-age report remains separate from citation/query semantics.
    pub fn run_research_mission_with_public_literature_freshness(
        &self,
        request: &CaseRequest,
        literature: &PublicLiteratureBundle,
        query: Option<&PublicLiteratureQuery>,
        freshness: Option<&RealDataFreshnessQuery>,
        max_steps: usize,
    ) -> Result<NeurosurgicalMissionResult, NeurosurgeryError> {
        self.run_research_mission_with_public_literature_portfolio(
            request, literature, query, freshness, None, max_steps,
        )
    }

    /// Compose a public-literature mission with an optional bounded multi-lane portfolio.
    /// The portfolio is computed from the same caller-supplied validated snapshot and remains a
    /// reviewer handoff; it never changes the single-specialty route or creates a clinical action.
    pub fn run_research_mission_with_public_literature_portfolio(
        &self,
        request: &CaseRequest,
        literature: &PublicLiteratureBundle,
        query: Option<&PublicLiteratureQuery>,
        freshness: Option<&RealDataFreshnessQuery>,
        portfolio_query: Option<&PublicLiteraturePortfolioQuery>,
        max_steps: usize,
    ) -> Result<NeurosurgicalMissionResult, NeurosurgeryError> {
        self.run_research_mission_with_public_literature_case_assets(
            request,
            literature,
            query,
            freshness,
            portfolio_query,
            None,
            None,
            max_steps,
        )
    }

    /// Compose a public-literature mission while attaching an optional de-identified multimodal
    /// asset manifest. Public citations and asset provenance remain separate report planes.
    #[allow(clippy::too_many_arguments)]
    pub fn run_research_mission_with_public_literature_case_assets(
        &self,
        request: &CaseRequest,
        literature: &PublicLiteratureBundle,
        query: Option<&PublicLiteratureQuery>,
        freshness: Option<&RealDataFreshnessQuery>,
        portfolio_query: Option<&PublicLiteraturePortfolioQuery>,
        case_asset_manifest: Option<&CaseAssetManifest>,
        case_asset_query: Option<&CaseAssetManifestQuery>,
        max_steps: usize,
    ) -> Result<NeurosurgicalMissionResult, NeurosurgeryError> {
        if case_asset_query.is_some() && case_asset_manifest.is_none() {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "a case asset manifest query requires a case asset manifest".to_string(),
            });
        }
        let case_asset_manifest = case_asset_manifest
            .map(|manifest| {
                let query = case_asset_query.cloned().unwrap_or_default();
                self.case_asset_manifest(request, manifest, &query)
            })
            .transpose()?;
        let mut packet_query = PublicLiteratureEvidencePacketQuery {
            query: query.cloned().unwrap_or_default(),
            freshness: freshness.cloned(),
        };
        if packet_query.query.specialty.is_none() {
            packet_query.query.specialty = Some(request.specialty);
        }
        let query_result = query
            .is_some()
            .then(|| literature.query(&packet_query.query))
            .transpose()?;
        // Run the lane-scoped completeness/identifier gate before assembling any local-model
        // handoff. Missing metadata remains review work, but it is visible at the start of the
        // mission rather than discovered after a packet has already been prepared.
        let public_literature_integrity_audit = Some(literature.integrity_audit(
            &PublicLiteratureIntegrityAuditQuery {
                specialties: Some(vec![request.specialty]),
                ..Default::default()
            },
        )?);
        let public_literature_review_queue =
            Some(literature.review_queue(&PublicLiteratureReviewQueueQuery {
                specialties: Some(vec![request.specialty]),
                ..Default::default()
            })?);
        let public_literature_workbench = Some(literature.specialty_workbench(
            &PublicLiteratureWorkbenchQuery {
                specialties: Some(vec![request.specialty]),
                freshness: freshness.cloned(),
                ..Default::default()
            },
        )?);
        let public_literature_portfolio = portfolio_query
            .map(|portfolio_query| literature.literature_portfolio(portfolio_query))
            .transpose()?;
        let context_query = PublicLiteratureReasoningContextQuery {
            packet: packet_query.clone(),
            ..PublicLiteratureReasoningContextQuery::default()
        };
        let public_literature_reasoning_context = literature.reasoning_context(&context_query)?;
        let public_literature_evidence_packet = Some(literature.evidence_packet(&packet_query)?);
        let public_literature_freshness = freshness
            .map(|query| literature.freshness_report(query))
            .transpose()?;
        let research_plan = Some(self.plan_research(
            request,
            None,
            Some(literature),
            MAX_RESEARCH_PLAN_TASKS,
            MAX_RESEARCH_PLAN_REFERENCES,
        )?);
        let evidence_program = Some(self.evidence_program_for_asset_report(
            request,
            None,
            Some(literature),
            case_asset_manifest.as_ref(),
            &EvidenceProgramQuery {
                freshness: freshness.cloned(),
                ..EvidenceProgramQuery::default()
            },
        )?);
        let acquisition_start = self.evidence_acquisition_start_with_case_assets(
            request,
            None,
            Some(literature),
            case_asset_manifest.as_ref(),
            &crate::EvidenceAcquisitionQuery {
                freshness: freshness.cloned(),
                ..crate::EvidenceAcquisitionQuery::default()
            },
        )?;
        let evidence_acquisition = Some(acquisition_start.plan);
        let evidence_acquisition_session = Some(acquisition_start.session);
        let research_brief = {
            let mut brief_query = NeurosurgicalResearchBriefQuery {
                public_literature_query: query.cloned(),
                freshness: freshness.cloned(),
                ..NeurosurgicalResearchBriefQuery::default()
            };
            if let Some(source_query) = brief_query.public_literature_query.as_mut() {
                if source_query.specialty.is_none() {
                    source_query.specialty = Some(request.specialty);
                }
            }
            Some(literature.research_brief(request, &brief_query)?)
        };
        let mut synthesis_query = EvidenceSynthesisQuery {
            public_literature_query: query.cloned(),
            freshness: freshness.cloned(),
            ..EvidenceSynthesisQuery::default()
        };
        if let Some(public_query) = synthesis_query.public_literature_query.as_mut() {
            if public_query.specialty.is_none() {
                public_query.specialty = Some(request.specialty);
            }
        }
        let evidence_synthesis = Some(self.evidence_synthesis_with_case_assets(
            request,
            None,
            Some(literature),
            &synthesis_query,
            case_asset_manifest.as_ref(),
        )?);
        let run =
            self.run_session_to_review_with_public_literature(request, literature, max_steps)?;
        let mission_id = format!(
            "neurosurgical-mission-{}",
            &run.response.request_digest[..16]
        );
        let mut mission = NeurosurgicalMissionResult {
            schema: "bioprism-neurosurgical-research-mission/0.1".to_string(),
            mission_id,
            specialty: request.specialty,
            status: run.response.status,
            human_review_required: true,
            provider: "none".to_string(),
            network: false,
            effects: vec![ToolEffect::ReadOnly],
            catalogue: MissionCatalogue {
                specialty_count: self.specialty_profiles().len(),
                tool_count: self.catalogue().len(),
            },
            case_asset_manifest,
            case_dicom_import: None,
            case_fhir_import: None,
            case_asset_review_disposition: None,
            real_data_query: None,
            public_literature_query: query_result,
            real_data_coverage: None,
            real_data_trial_landscape: None,
            real_data_molecular_coverage: None,
            real_data_cohort_landscape: None,
            specialty_evidence_map: Some(self.specialty_evidence_map(request)?),
            real_data_review_queue: None,
            real_data_evidence_packet: None,
            real_data_autonomous_workflow: None,
            real_data_freshness: None,
            real_data_evidence_graph: None,
            real_data_reasoning_context: None,
            public_literature_reasoning_context: Some(public_literature_reasoning_context),
            public_literature_evidence_packet,
            public_literature_freshness,
            public_literature_integrity_audit,
            public_literature_review_queue,
            public_literature_workbench,
            public_literature_portfolio,
            literature_link_audit: None,
            evidence_synthesis,
            research_plan,
            evidence_program,
            evidence_acquisition,
            evidence_acquisition_session,
            research_brief,
            mission_audit: None,
            run,
        };
        mission.mission_audit = Some(crate::audit_mission(
            &mission,
            request,
            None,
            Some(literature),
        )?);
        Ok(mission)
    }

    /// Rebuild a public-literature mission with an optional persisted case-asset disposition
    /// ledger. This preserves the public citation planes while rebinding synthesis, evidence
    /// programming, acquisition, and audit state.
    #[allow(clippy::too_many_arguments)]
    pub fn run_research_mission_with_public_literature_case_assets_and_dispositions(
        &self,
        request: &CaseRequest,
        literature: &PublicLiteratureBundle,
        query: Option<&PublicLiteratureQuery>,
        freshness: Option<&RealDataFreshnessQuery>,
        portfolio_query: Option<&PublicLiteraturePortfolioQuery>,
        case_asset_manifest: Option<&CaseAssetManifest>,
        case_asset_query: Option<&CaseAssetManifestQuery>,
        case_asset_disposition: Option<&CaseAssetReviewDispositionReport>,
        max_steps: usize,
    ) -> Result<NeurosurgicalMissionResult, NeurosurgeryError> {
        let mut mission = self.run_research_mission_with_public_literature_case_assets(
            request,
            literature,
            query,
            freshness,
            portfolio_query,
            case_asset_manifest,
            case_asset_query,
            max_steps,
        )?;
        if let Some(disposition) = case_asset_disposition {
            let mut synthesis_query = EvidenceSynthesisQuery {
                public_literature_query: query.cloned(),
                freshness: freshness.cloned(),
                ..EvidenceSynthesisQuery::default()
            };
            if let Some(public_query) = synthesis_query.public_literature_query.as_mut() {
                if public_query.specialty.is_none() {
                    public_query.specialty = Some(request.specialty);
                }
            }
            mission.evidence_synthesis =
                Some(self.evidence_synthesis_with_case_assets_and_dispositions(
                    request,
                    None,
                    Some(literature),
                    &synthesis_query,
                    mission.case_asset_manifest.as_ref(),
                    Some(disposition),
                )?);
            mission.evidence_program =
                Some(self.evidence_program_for_asset_report_and_dispositions(
                    request,
                    None,
                    Some(literature),
                    mission.case_asset_manifest.as_ref(),
                    disposition,
                    &EvidenceProgramQuery {
                        freshness: freshness.cloned(),
                        ..EvidenceProgramQuery::default()
                    },
                )?);
            let acquisition_start = self
                .evidence_acquisition_start_with_case_assets_and_dispositions(
                    request,
                    None,
                    Some(literature),
                    mission.case_asset_manifest.as_ref(),
                    disposition,
                    &crate::EvidenceAcquisitionQuery {
                        freshness: freshness.cloned(),
                        ..crate::EvidenceAcquisitionQuery::default()
                    },
                )?;
            mission.evidence_acquisition = Some(acquisition_start.plan);
            mission.evidence_acquisition_session = Some(acquisition_start.session);
            mission.case_asset_review_disposition = Some(disposition.clone());
            mission.mission_audit = Some(crate::audit_mission(
                &mission,
                request,
                None,
                Some(literature),
            )?);
        }
        Ok(mission)
    }

    /// Compose a glioma mission from both validated public bundles: the real glioma registry /
    /// genomics snapshot remains the mission's population-data route, while the cross-specialty
    /// PubMed snapshot supplies citation context. Exact PMID/DOI linkage is returned separately
    /// so a caller can audit identity without treating records as the same cohort. The two
    /// bundles are never merged, rewritten, or fetched, and both validators run before output.
    #[allow(clippy::too_many_arguments)]
    pub fn run_research_mission_with_real_data_and_public_literature(
        &self,
        request: &CaseRequest,
        real_data: &RealGliomaBundle,
        literature: &PublicLiteratureBundle,
        real_query: Option<&RealDataQuery>,
        public_query: Option<&PublicLiteratureQuery>,
        freshness: Option<&RealDataFreshnessQuery>,
        portfolio_query: Option<&PublicLiteraturePortfolioQuery>,
        max_steps: usize,
    ) -> Result<NeurosurgicalMissionResult, NeurosurgeryError> {
        self.run_research_mission_with_real_data_and_public_literature_case_assets(
            request,
            real_data,
            literature,
            real_query,
            public_query,
            freshness,
            portfolio_query,
            None,
            None,
            max_steps,
        )
    }

    /// Compose a dual-bundle glioma mission and attach a de-identified multimodal asset manifest
    /// to the real-data mission plane. Population and citation bundles remain independently
    /// digest-bound; asset bytes are never opened and the session route is unchanged.
    #[allow(clippy::too_many_arguments)]
    pub fn run_research_mission_with_real_data_and_public_literature_case_assets(
        &self,
        request: &CaseRequest,
        real_data: &RealGliomaBundle,
        literature: &PublicLiteratureBundle,
        real_query: Option<&RealDataQuery>,
        public_query: Option<&PublicLiteratureQuery>,
        freshness: Option<&RealDataFreshnessQuery>,
        portfolio_query: Option<&PublicLiteraturePortfolioQuery>,
        case_asset_manifest: Option<&CaseAssetManifest>,
        case_asset_query: Option<&CaseAssetManifestQuery>,
        max_steps: usize,
    ) -> Result<NeurosurgicalMissionResult, NeurosurgeryError> {
        // Keep the real-data mission as the canonical route because the glioma specialty requires
        // it. The public pass contributes only its independently digest-bound reports; this keeps
        // the session event chain single-source while exposing both evidence planes.
        let mut mission = self.run_research_mission_with_case_assets(
            request,
            Some(real_data),
            real_query,
            freshness,
            case_asset_manifest,
            case_asset_query,
            max_steps,
        )?;
        let public_mission = self.run_research_mission_with_public_literature_portfolio(
            request,
            literature,
            public_query,
            freshness,
            portfolio_query,
            max_steps,
        )?;
        let mut synthesis_query = EvidenceSynthesisQuery {
            real_data_query: real_query.cloned(),
            public_literature_query: public_query.cloned(),
            freshness: freshness.cloned(),
            ..EvidenceSynthesisQuery::default()
        };
        if let Some(public_query) = synthesis_query.public_literature_query.as_mut() {
            if public_query.specialty.is_none() {
                public_query.specialty = Some(request.specialty);
            }
        }
        mission.evidence_synthesis = Some(self.evidence_synthesis_with_case_assets(
            request,
            Some(real_data),
            Some(literature),
            &synthesis_query,
            mission.case_asset_manifest.as_ref(),
        )?);
        mission.evidence_program = Some(self.evidence_program_for_asset_report(
            request,
            Some(real_data),
            Some(literature),
            mission.case_asset_manifest.as_ref(),
            &EvidenceProgramQuery {
                freshness: freshness.cloned(),
                ..EvidenceProgramQuery::default()
            },
        )?);
        let acquisition_start = self.evidence_acquisition_start_with_case_assets(
            request,
            Some(real_data),
            Some(literature),
            mission.case_asset_manifest.as_ref(),
            &crate::EvidenceAcquisitionQuery {
                freshness: freshness.cloned(),
                ..crate::EvidenceAcquisitionQuery::default()
            },
        )?;
        mission.evidence_acquisition = Some(acquisition_start.plan);
        mission.evidence_acquisition_session = Some(acquisition_start.session);
        mission.public_literature_query = public_mission.public_literature_query;
        mission.public_literature_reasoning_context =
            public_mission.public_literature_reasoning_context;
        mission.public_literature_evidence_packet =
            public_mission.public_literature_evidence_packet;
        mission.public_literature_freshness = public_mission.public_literature_freshness;
        mission.public_literature_integrity_audit =
            public_mission.public_literature_integrity_audit;
        mission.public_literature_review_queue = public_mission.public_literature_review_queue;
        mission.public_literature_workbench = public_mission.public_literature_workbench;
        mission.public_literature_portfolio = public_mission.public_literature_portfolio;
        mission.literature_link_audit = Some(
            real_data.literature_link_audit(literature, &LiteratureLinkAuditQuery::default())?,
        );
        mission.mission_audit = Some(crate::audit_mission(
            &mission,
            request,
            Some(real_data),
            Some(literature),
        )?);
        Ok(mission)
    }

    /// Rebuild a dual-bundle glioma mission with a persisted case-asset disposition ledger. The
    /// two public evidence planes remain independent; synthesis, evidence programming,
    /// acquisition, and audit are rebound to the reviewer state after the canonical mission
    /// constructor completes.
    #[allow(clippy::too_many_arguments)]
    pub fn run_research_mission_with_real_data_and_public_literature_case_assets_and_dispositions(
        &self,
        request: &CaseRequest,
        real_data: &RealGliomaBundle,
        literature: &PublicLiteratureBundle,
        real_query: Option<&RealDataQuery>,
        public_query: Option<&PublicLiteratureQuery>,
        freshness: Option<&RealDataFreshnessQuery>,
        portfolio_query: Option<&PublicLiteraturePortfolioQuery>,
        case_asset_manifest: Option<&CaseAssetManifest>,
        case_asset_query: Option<&CaseAssetManifestQuery>,
        case_asset_disposition: Option<&CaseAssetReviewDispositionReport>,
        max_steps: usize,
    ) -> Result<NeurosurgicalMissionResult, NeurosurgeryError> {
        let mut mission = self
            .run_research_mission_with_real_data_and_public_literature_case_assets(
                request,
                real_data,
                literature,
                real_query,
                public_query,
                freshness,
                portfolio_query,
                case_asset_manifest,
                case_asset_query,
                max_steps,
            )?;
        if let Some(disposition) = case_asset_disposition {
            let mut synthesis_query = EvidenceSynthesisQuery {
                real_data_query: real_query.cloned(),
                public_literature_query: public_query.cloned(),
                freshness: freshness.cloned(),
                ..EvidenceSynthesisQuery::default()
            };
            if let Some(public_query) = synthesis_query.public_literature_query.as_mut() {
                if public_query.specialty.is_none() {
                    public_query.specialty = Some(request.specialty);
                }
            }
            mission.evidence_synthesis =
                Some(self.evidence_synthesis_with_case_assets_and_dispositions(
                    request,
                    Some(real_data),
                    Some(literature),
                    &synthesis_query,
                    mission.case_asset_manifest.as_ref(),
                    Some(disposition),
                )?);
            mission.evidence_program =
                Some(self.evidence_program_for_asset_report_and_dispositions(
                    request,
                    Some(real_data),
                    Some(literature),
                    mission.case_asset_manifest.as_ref(),
                    disposition,
                    &EvidenceProgramQuery {
                        freshness: freshness.cloned(),
                        ..EvidenceProgramQuery::default()
                    },
                )?);
            let acquisition_start = self
                .evidence_acquisition_start_with_case_assets_and_dispositions(
                    request,
                    Some(real_data),
                    Some(literature),
                    mission.case_asset_manifest.as_ref(),
                    disposition,
                    &crate::EvidenceAcquisitionQuery {
                        freshness: freshness.cloned(),
                        ..crate::EvidenceAcquisitionQuery::default()
                    },
                )?;
            mission.evidence_acquisition = Some(acquisition_start.plan);
            mission.evidence_acquisition_session = Some(acquisition_start.session);
            mission.case_asset_review_disposition = Some(disposition.clone());
            mission.mission_audit = Some(crate::audit_mission(
                &mission,
                request,
                Some(real_data),
                Some(literature),
            )?);
        }
        Ok(mission)
    }

    /// Finish a fully advanced session and reconstruct the ordinary report. The report is
    /// recomputed from the caller's request and bundle, then checked against every session event
    /// so a tampered checkpoint cannot be promoted to a final response.
    pub fn finish_session(
        &self,
        session: &NeurosurgicalSession,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
    ) -> Result<AgentResponse, NeurosurgeryError> {
        self.validate_session(session)?;
        let (prepared, summary) = self.prepare_request(request, real_data)?;
        self.finish_session_prepared(
            session,
            &prepared,
            SessionEvidence {
                real_data,
                real_summary: summary.as_ref(),
                public_literature: None,
                public_summary: None,
                original_request: request,
            },
        )
    }

    /// Finish a public-literature-backed session after every route step is checkpointed.
    pub fn finish_session_with_public_literature(
        &self,
        session: &NeurosurgicalSession,
        request: &CaseRequest,
        literature: &PublicLiteratureBundle,
    ) -> Result<AgentResponse, NeurosurgeryError> {
        self.validate_session(session)?;
        let (prepared, summary) = self.prepare_public_literature_request(request, literature)?;
        self.finish_session_prepared(
            session,
            &prepared,
            SessionEvidence {
                real_data: None,
                real_summary: None,
                public_literature: Some(literature),
                public_summary: Some(&summary),
                original_request: request,
            },
        )
    }

    fn finish_session_prepared(
        &self,
        session: &NeurosurgicalSession,
        prepared: &CaseRequest,
        evidence: SessionEvidence<'_>,
    ) -> Result<AgentResponse, NeurosurgeryError> {
        self.assert_session_inputs(
            session,
            prepared,
            evidence.real_summary,
            evidence.public_summary,
        )?;
        if session.next_ordinal as usize <= session.route.len() {
            return Err(NeurosurgeryError::SessionRejected {
                reason: "session must advance every route step before finish".to_string(),
            });
        }
        let response = if let Some(real_data) = evidence.real_data {
            self.run_with_real_glioma_data(evidence.original_request, real_data)?
        } else if let Some(literature) = evidence.public_literature {
            self.run_with_public_literature(evidence.original_request, literature)?
        } else {
            self.run(prepared)?
        };
        response
            .validate_integrity()
            .map_err(|_| NeurosurgeryError::SessionRejected {
                reason: "final agent response failed its integrity contract".to_string(),
            })?;
        if response.request_digest != session.request_digest {
            return Err(NeurosurgeryError::SessionRejected {
                reason: "final response request digest does not match session".to_string(),
            });
        }
        if response.tool_runs.len() != session.events.len()
            || response
                .tool_runs
                .iter()
                .zip(&session.events)
                .any(|(run, event)| {
                    run.capability != event.capability
                        || run.status != event.status
                        || digest_value(run).map_or(true, |digest| digest != event.finding_digest)
                })
        {
            return Err(NeurosurgeryError::SessionRejected {
                reason: "final tool runs do not match the session event chain".to_string(),
            });
        }
        Ok(response)
    }

    /// Parses a request and a real-data bundle from JSON documents without provider coupling.
    pub fn run_json_with_real_glioma_data(
        &self,
        request_document: &Value,
        data_document: &Value,
    ) -> Result<AgentResponse, NeurosurgeryError> {
        let request: CaseRequest = serde_json::from_value(request_document.clone())
            .map_err(|error| NeurosurgeryError::Json(error.to_string()))?;
        let data: RealGliomaBundle = serde_json::from_value(data_document.clone())
            .map_err(|error| NeurosurgeryError::Json(error.to_string()))?;
        self.run_with_real_glioma_data(&request, &data)
    }

    fn prepare_request(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
    ) -> Result<(CaseRequest, Option<RealDataSummary>), NeurosurgeryError> {
        self.validate_request(request)?;
        let Some(data) = real_data else {
            if request.requested_tools.iter().any(|tool| {
                matches!(
                    tool,
                    ToolCapability::RealDataInventory | ToolCapability::RealDataQuery
                )
            }) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: "real_data_inventory requires a validated public glioma bundle"
                        .to_string(),
                });
            }
            return Ok((request.clone(), None));
        };
        if request.specialty != Specialty::Glioma {
            return Err(NeurosurgeryError::RealDataSpecialtyUnsupported {
                specialty: request.specialty,
            });
        }
        if request.request_use == RequestUse::SyntheticCaseSimulation {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "synthetic_case_simulation cannot be combined with a real-data run"
                    .to_string(),
            });
        }
        if contains_synthetic_marker(&request.case_id)
            || contains_synthetic_marker(&request.question)
            || request.observations.iter().any(|observation| {
                contains_synthetic_marker(&observation.label)
                    || contains_synthetic_marker(&observation.value)
                    || observation
                        .source_id
                        .as_deref()
                        .is_some_and(contains_synthetic_marker)
            })
            || request.evidence.iter().any(|record| {
                contains_synthetic_marker(&record.id)
                    || contains_synthetic_marker(&record.title)
                    || contains_synthetic_marker(&record.citation)
                    || record
                        .population
                        .as_deref()
                        .is_some_and(contains_synthetic_marker)
            })
            || request
                .glioma_molecular
                .as_ref()
                .is_some_and(GliomaMolecularPanel::contains_synthetic_marker)
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason:
                    "synthetic observation or evidence marker is not accepted in a real-data run"
                        .to_string(),
            });
        }
        let summary = data.summary()?;
        let mut enriched = request.clone();
        if enriched
            .requested_tools
            .contains(&ToolCapability::RealDataQuery)
        {
            let Some(query) = enriched.real_data_query.as_ref() else {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: "real_data_query requires an explicit real_data_query object"
                        .to_string(),
                });
            };
            data.query(query)?;
        }
        let existing_ids = enriched
            .evidence
            .iter()
            .map(|record| record.id.clone())
            .collect::<BTreeSet<_>>();
        enriched.evidence.extend(
            data.evidence_records()
                .into_iter()
                .filter(|record| !existing_ids.contains(&record.id)),
        );
        if !enriched
            .requested_tools
            .contains(&ToolCapability::RealDataInventory)
        {
            if enriched.requested_tools.len() >= self.max_requested_tools {
                return Err(NeurosurgeryError::TooMany {
                    field: "requested_tools",
                    found: enriched.requested_tools.len() + 1,
                    max: self.max_requested_tools,
                });
            }
            enriched
                .requested_tools
                .push(ToolCapability::RealDataInventory);
        }
        Ok((enriched, Some(summary)))
    }

    fn validate_session(&self, session: &NeurosurgicalSession) -> Result<(), NeurosurgeryError> {
        if session.schema_version != NEUROSURGERY_SCHEMA_VERSION
            || session.session_id.trim().is_empty()
            || session.request_digest.len() != 64
            || !session
                .request_digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || session.session_id != format!("ns-session-{}", &session.request_digest[..16])
            || session.route.is_empty()
            || session.route.first() != Some(&ToolCapability::SafetyGate)
            || session.route.last() != Some(&ToolCapability::HumanReviewHold)
            || session.events.len() > session.route.len()
            || session.next_ordinal as usize != session.events.len() + 1
        {
            return Err(NeurosurgeryError::SessionRejected {
                reason: "session envelope or route invariants are invalid".to_string(),
            });
        }
        let known_tools = self
            .catalogue()
            .into_iter()
            .map(|spec| spec.capability)
            .collect::<BTreeSet<_>>();
        let mut route_seen = BTreeSet::new();
        if session
            .route
            .iter()
            .any(|capability| !known_tools.contains(capability) || !route_seen.insert(*capability))
        {
            return Err(NeurosurgeryError::SessionRejected {
                reason: "session route contains an unknown or duplicate tool".to_string(),
            });
        }
        let initial_chain = digest_value(&(
            session.session_id.as_str(),
            session.request_digest.as_str(),
            &session.route,
        ))?;
        let mut previous = initial_chain;
        for (index, event) in session.events.iter().enumerate() {
            if event.previous_event_digest != previous
                || event.ordinal as usize != index + 1
                || session.route[index] != event.capability
                || event.finding_digest.len() != 64
                || !event
                    .finding_digest
                    .bytes()
                    .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
                || event.event_digest
                    != session_event_digest(
                        event.ordinal,
                        event.capability,
                        event.status,
                        &event.finding_digest,
                        &event.previous_event_digest,
                    )?
                || (event.capability == ToolCapability::HumanReviewHold
                    && event.status != ToolRunStatus::HeldForHumanReview)
                || (event.capability != ToolCapability::HumanReviewHold
                    && event.status == ToolRunStatus::HeldForHumanReview)
            {
                return Err(NeurosurgeryError::SessionRejected {
                    reason: "session event chain is invalid or tampered".to_string(),
                });
            }
            previous = event.event_digest.clone();
        }
        if session.event_chain_digest != previous {
            return Err(NeurosurgeryError::SessionRejected {
                reason: "session chain digest does not match events".to_string(),
            });
        }
        let expected_status = match session.events.last() {
            None => SessionStatus::Planned,
            Some(event) if event.capability == ToolCapability::HumanReviewHold => {
                SessionStatus::AwaitingHumanReview
            }
            Some(event) if event.status == ToolRunStatus::NeedsInput => SessionStatus::NeedsInput,
            Some(_) => SessionStatus::Running,
        };
        if session.status != expected_status {
            return Err(NeurosurgeryError::SessionRejected {
                reason: "session status does not match its checkpoint events".to_string(),
            });
        }
        Ok(())
    }

    /// Validate a caller-persisted session checkpoint without advancing it.
    ///
    /// This is intentionally read-only: it checks the route, event chain, terminal state, and
    /// digest shape, but never opens a bundle, invokes a provider, or mutates the checkpoint.
    /// Callers that need exact request/snapshot rebinding should use the corresponding advance
    /// or finish operation, which additionally checks those input digests.
    pub fn validate_session_integrity(
        &self,
        session: &NeurosurgicalSession,
    ) -> Result<(), NeurosurgeryError> {
        self.validate_session(session)
    }

    fn assert_session_inputs(
        &self,
        session: &NeurosurgicalSession,
        prepared: &CaseRequest,
        real_summary: Option<&RealDataSummary>,
        public_summary: Option<&PublicLiteratureSummary>,
    ) -> Result<(), NeurosurgeryError> {
        let expected_route = self.route(
            &required_capabilities(prepared.specialty),
            &prepared.requested_tools,
        )?;
        if session.specialty != prepared.specialty
            || session.route != expected_route
            || digest(prepared)? != session.request_digest
            || real_summary.map(|summary| summary.bundle_digest.as_str())
                != session.real_data_digest.as_deref()
            || public_summary.map(|summary| summary.bundle_digest.as_str())
                != session.public_literature_digest.as_deref()
        {
            return Err(NeurosurgeryError::SessionRejected {
                reason: "session input digest does not match the supplied request or data"
                    .to_string(),
            });
        }
        Ok(())
    }

    /// Parses and runs a JSON request, keeping provider and credential concerns outside the crate.
    pub fn run_json(&self, document: &Value) -> Result<AgentResponse, NeurosurgeryError> {
        let request: CaseRequest = serde_json::from_value(document.clone())
            .map_err(|error| NeurosurgeryError::Json(error.to_string()))?;
        self.run(&request)
    }

    fn validate_request(&self, request: &CaseRequest) -> Result<(), NeurosurgeryError> {
        if request.schema_version != NEUROSURGERY_SCHEMA_VERSION {
            return Err(NeurosurgeryError::UnsupportedSchema {
                found: request.schema_version.clone(),
                expected: NEUROSURGERY_SCHEMA_VERSION,
            });
        }
        validate_text(&request.case_id, "case_id", MAX_CASE_ID_BYTES)?;
        validate_text(&request.question, "question", MAX_QUESTION_BYTES)?;
        if request.request_use.is_clinical() {
            return Err(NeurosurgeryError::ClinicalUseRefused {
                use_case: request.request_use,
                description: request.request_use.description(),
            });
        }
        if !request.direct_identifier_fields.is_empty() {
            return Err(NeurosurgeryError::DirectIdentifiers {
                fields: request.direct_identifier_fields.clone(),
            });
        }
        if request.observations.len() > self.max_observations {
            return Err(NeurosurgeryError::TooMany {
                field: "observations",
                found: request.observations.len(),
                max: self.max_observations,
            });
        }
        if request.evidence.len() > self.max_evidence {
            return Err(NeurosurgeryError::TooMany {
                field: "evidence",
                found: request.evidence.len(),
                max: self.max_evidence,
            });
        }
        if request.requested_tools.len() > self.max_requested_tools {
            return Err(NeurosurgeryError::TooMany {
                field: "requested_tools",
                found: request.requested_tools.len(),
                max: self.max_requested_tools,
            });
        }
        let mut seen = BTreeSet::new();
        for tool in &request.requested_tools {
            if !seen.insert(*tool) {
                return Err(NeurosurgeryError::DuplicateTool { tool: *tool });
            }
        }
        for observation in &request.observations {
            validate_text(&observation.label, "observation.label", MAX_TEXT_BYTES)?;
            validate_text(&observation.value, "observation.value", MAX_TEXT_BYTES)?;
            if let Some(source_id) = &observation.source_id {
                validate_text(source_id, "observation.source_id", MAX_CASE_ID_BYTES)?;
            }
            if let Some(observed_at) = &observation.observed_at {
                validate_text(observed_at, "observation.observed_at", 32)?;
                if !crate::temporal::is_utc_timestamp(observed_at) {
                    return Err(NeurosurgeryError::TemporalRejected {
                        reason: "observation.observed_at must be a UTC RFC3339 timestamp"
                            .to_string(),
                    });
                }
            }
            if let Some(timepoint) = &observation.timepoint {
                validate_text(timepoint, "observation.timepoint", 128)?;
            }
        }
        for evidence in &request.evidence {
            validate_text(&evidence.id, "evidence.id", MAX_CASE_ID_BYTES)?;
            validate_text(&evidence.title, "evidence.title", MAX_TEXT_BYTES)?;
            validate_text(&evidence.citation, "evidence.citation", MAX_TEXT_BYTES)?;
            if evidence.id.trim().is_empty()
                || evidence.title.trim().is_empty()
                || evidence.citation.trim().is_empty()
            {
                return Err(NeurosurgeryError::InvalidEvidence {
                    id: evidence.id.clone(),
                });
            }
            if let Some(population) = &evidence.population {
                validate_text(population, "evidence.population", MAX_TEXT_BYTES)?;
            }
        }
        if let Some(panel) = &request.glioma_molecular {
            if request.specialty != Specialty::Glioma {
                return Err(NeurosurgeryError::GliomaPanelRejected {
                    reason: format!(
                        "glioma_molecular is only accepted for glioma requests, not {:?}",
                        request.specialty
                    ),
                });
            }
            panel.validate()?;
        }
        Ok(())
    }

    fn route(
        &self,
        required: &[ToolCapability],
        requested: &[ToolCapability],
    ) -> Result<Vec<ToolCapability>, NeurosurgeryError> {
        let catalogue = self.catalogue();
        let available = catalogue
            .iter()
            .map(|tool| tool.capability)
            .collect::<BTreeSet<_>>();
        let mut route = required.to_vec();
        for requested_tool in requested {
            if !available.contains(requested_tool) {
                return Err(NeurosurgeryError::UnknownTool {
                    tool: *requested_tool,
                });
            }
            if !route.contains(requested_tool) {
                route.push(*requested_tool);
            }
        }
        // A caller may ask for additional read-only checks, but cannot move the safety gate or
        // human hold. Re-establish those sentinels after extending the route.
        route.retain(|tool| {
            *tool != ToolCapability::SafetyGate && *tool != ToolCapability::HumanReviewHold
        });
        if let Some(inventory_index) = route
            .iter()
            .position(|tool| *tool == ToolCapability::RealDataInventory)
        {
            let inventory = route.remove(inventory_index);
            let evidence_index = route
                .iter()
                .position(|tool| *tool == ToolCapability::EvidenceSynthesis)
                .unwrap_or(route.len());
            route.insert(evidence_index, inventory);
        }
        if let Some(query_index) = route
            .iter()
            .position(|tool| *tool == ToolCapability::RealDataQuery)
        {
            let query = route.remove(query_index);
            let evidence_index = route
                .iter()
                .position(|tool| *tool == ToolCapability::EvidenceSynthesis)
                .unwrap_or(route.len());
            route.insert(evidence_index, query);
        }
        route.insert(0, ToolCapability::SafetyGate);
        route.push(ToolCapability::HumanReviewHold);
        Ok(route)
    }
}

impl AgentResponse {
    /// Validate a persisted terminal response without reopening the original request.
    ///
    /// This checks the closed route, tool trace, evidence-gap projection, nested provenance
    /// reports, and response digest. It does not assert that caller-supplied observations are
    /// clinically true or sufficient.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        if self.schema_version != NEUROSURGERY_SCHEMA_VERSION
            || !is_sha256_hex(&self.response_digest)
            || !is_sha256_hex(&self.request_digest)
            || self.specialty_profile.specialty != self.specialty
            || self.plan.len() != self.tool_runs.len()
            || self.plan.is_empty()
            || self.plan.first().map(|step| step.capability) != Some(ToolCapability::SafetyGate)
            || self.plan.last().map(|step| step.capability) != Some(ToolCapability::HumanReviewHold)
        {
            return Err(response_rejected("agent response envelope is invalid"));
        }
        let known_tools = tool_catalogue()
            .into_iter()
            .map(|spec| spec.capability)
            .collect::<BTreeSet<_>>();
        let mut seen = BTreeSet::new();
        for (index, (step, run)) in self.plan.iter().zip(self.tool_runs.iter()).enumerate() {
            if step.ordinal as usize != index + 1
                || step.capability != run.capability
                || !known_tools.contains(&step.capability)
                || !seen.insert(step.capability)
                || step.purpose != tool_spec(step.capability).purpose
                || step.effect != ToolEffect::ReadOnly
                || !step.requires_human_review
                || (step.capability == ToolCapability::HumanReviewHold
                    && run.status != ToolRunStatus::HeldForHumanReview)
                || (step.capability != ToolCapability::HumanReviewHold
                    && run.status == ToolRunStatus::HeldForHumanReview)
                || run.findings.iter().any(|finding| {
                    finding.code.trim().is_empty() || finding.detail.trim().is_empty() || {
                        let mut evidence_ids = BTreeSet::new();
                        finding.evidence_ids.iter().any(|evidence_id| {
                            evidence_id.trim().is_empty() || !evidence_ids.insert(evidence_id)
                        })
                    }
                })
            {
                return Err(response_rejected(
                    "agent response route or tool trace is invalid",
                ));
            }
        }
        let required = required_capabilities(self.specialty);
        let mut required_index = 0usize;
        for capability in self.plan.iter().map(|step| step.capability) {
            if required_index < required.len() && capability == required[required_index] {
                required_index += 1;
            }
        }
        if required_index != required.len() {
            return Err(response_rejected(
                "agent response route omits a mandatory specialty capability",
            ));
        }
        let mut gap_capabilities = BTreeSet::new();
        if self.evidence_gaps.iter().any(|gap| {
            !seen.contains(&gap.capability)
                || !gap_capabilities.insert(gap.capability)
                || gap.reason.trim().is_empty()
        }) {
            return Err(response_rejected(
                "agent response evidence gaps are invalid",
            ));
        }
        let expected_status = if self.evidence_gaps.is_empty() {
            AgentStatus::ReadyForHumanReview
        } else {
            AgentStatus::NeedsEvidence
        };
        if self.status != expected_status
            || self.tool_runs.last().map(|run| run.status)
                != Some(ToolRunStatus::HeldForHumanReview)
            || self.report.non_clinical_use_notice.trim().is_empty()
            || self.report.scope.trim().is_empty()
            || self.report.prohibited_actions.is_empty()
        {
            return Err(response_rejected(
                "agent response status or research report boundary is invalid",
            ));
        }
        if let Some(temporal) = self.temporal_alignment.as_ref() {
            if temporal.schema_version != crate::temporal::TEMPORAL_ALIGNMENT_SCHEMA_VERSION
                || temporal.request_digest != self.request_digest
                || temporal.specialty != self.specialty
                || temporal.observation_count != temporal.observations.len()
                || temporal.timestamped_observation_count + temporal.untimestamped_observation_count
                    != temporal.observation_count
                || !temporal.human_review_required
                || temporal.provider != "none"
                || temporal.network
                || temporal.effect != "read_only"
            {
                return Err(response_rejected(
                    "agent response temporal alignment is invalid",
                ));
            }
        }
        if let Some(map) = self.specialty_evidence_map.as_ref() {
            if map.specialty != self.specialty
                || map.request_digest != self.request_digest
                || map.validate_integrity().is_err()
            {
                return Err(response_rejected(
                    "agent response specialty evidence map is invalid",
                ));
            }
        } else {
            return Err(response_rejected(
                "agent response is missing its specialty evidence map",
            ));
        }
        if self.real_data.as_ref().is_some_and(|summary| {
            summary.bundle_schema_version != crate::real_data::REAL_DATA_SCHEMA_VERSION
                || !is_sha256_hex(&summary.bundle_digest)
                || !summary.provenance_bound
                || summary.synthetic_data
                || (!summary.genomic_project_case_counts.is_empty()
                    && (summary.genomic_project_case_counts.len() != summary.genomic_project_count
                        || summary.genomic_project_case_counts.iter().any(|entry| {
                            entry.project_id.trim().is_empty() || entry.case_count == 0
                        })
                        || summary
                            .genomic_project_case_counts
                            .windows(2)
                            .any(|window| window[0].project_id >= window[1].project_id)
                        || summary
                            .genomic_project_case_counts
                            .iter()
                            .map(|entry| entry.case_count)
                            .sum::<usize>()
                            != summary.genomic_case_count))
                || summary
                    .genomic_project_data_type_counts
                    .iter()
                    .any(|entry| {
                        entry.project_id.trim().is_empty()
                            || entry.data_type.trim().is_empty()
                            || entry.file_count == 0
                            || !summary
                                .genomic_project_case_counts
                                .iter()
                                .any(|project| project.project_id == entry.project_id)
                    })
                || summary
                    .genomic_project_data_type_counts
                    .windows(2)
                    .any(|window| {
                        (window[0].project_id.as_str(), window[0].data_type.as_str())
                            >= (window[1].project_id.as_str(), window[1].data_type.as_str())
                    })
        }) || self.public_literature.as_ref().is_some_and(|summary| {
            summary.schema_version != crate::public_literature::PUBLIC_LITERATURE_SCHEMA_VERSION
                || !is_sha256_hex(&summary.bundle_digest)
                || !summary.provenance_bound
                || summary.synthetic_data
        }) || self.glioma_molecular.as_ref().is_some_and(|summary| {
            self.specialty != Specialty::Glioma
                || summary.schema_version != crate::GLIOMA_MOLECULAR_SCHEMA_VERSION
                || !is_sha256_hex(&summary.panel_digest)
                || summary.marker_count != summary.markers.len()
        }) {
            return Err(response_rejected(
                "agent response nested provenance summary is invalid",
            ));
        }
        if self.response_digest != digest_response(self)? {
            return Err(response_rejected(
                "agent response digest does not match its contents",
            ));
        }
        Ok(())
    }

    /// Validate a response against the exact prepared request used to execute its route.
    /// Real/public wrapper methods pass their enriched request through this same boundary before
    /// returning the response; callers can use this when replaying a persisted plain run.
    pub fn validate_for_request(&self, request: &CaseRequest) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        if self.specialty != request.specialty || self.request_digest != digest(request)? {
            return Err(response_rejected(
                "agent response is not bound to the supplied request",
            ));
        }
        Ok(())
    }
}

fn response_rejected(reason: &str) -> NeurosurgeryError {
    NeurosurgeryError::RealDataRejected {
        reason: reason.to_string(),
    }
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
}

fn digest_response(response: &AgentResponse) -> Result<String, NeurosurgeryError> {
    let mut unsigned = response.clone();
    unsigned.response_digest.clear();
    digest_value(&unsigned)
}

fn seal_response(response: &mut AgentResponse) -> Result<(), NeurosurgeryError> {
    response.response_digest = digest_response(response)?;
    response.validate_integrity()
}

fn validate_text(value: &str, field: &'static str, max: usize) -> Result<(), NeurosurgeryError> {
    if value.trim().is_empty() {
        return Err(NeurosurgeryError::EmptyField { field });
    }
    if value.len() > max {
        return Err(NeurosurgeryError::FieldTooLong { field, max });
    }
    if value.chars().any(char::is_control) {
        return Err(NeurosurgeryError::ControlCharacter { field });
    }
    Ok(())
}

fn contains_synthetic_marker(value: &str) -> bool {
    value.to_ascii_lowercase().contains("synthetic")
}

fn digest(request: &CaseRequest) -> Result<String, NeurosurgeryError> {
    digest_value(request)
}

fn digest_value<T: serde::Serialize>(value: &T) -> Result<String, NeurosurgeryError> {
    let bytes =
        serde_json::to_vec(value).map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn session_event_digest(
    ordinal: u16,
    capability: ToolCapability,
    status: ToolRunStatus,
    finding_digest: &str,
    previous_event_digest: &str,
) -> Result<String, NeurosurgeryError> {
    digest_value(&(
        ordinal,
        capability,
        status,
        finding_digest,
        previous_event_digest,
    ))
}

fn evidence_gaps(request: &CaseRequest, required: &[ToolCapability]) -> Vec<EvidenceGap> {
    let mut gaps = Vec::new();
    for capability in required {
        let state = match capability {
            ToolCapability::SafetyGate
            | ToolCapability::EvidenceGapScan
            | ToolCapability::DifferentialMatrix
            | ToolCapability::HumanReviewHold => None,
            ToolCapability::CaseIntegrity => {
                if request
                    .observations
                    .iter()
                    .any(|observation| observation.source_id.is_none())
                {
                    Some((
                        EvidenceState::Unmeasured,
                        "one or more observations has no provenance source_id".to_string(),
                    ))
                } else {
                    None
                }
            }
            ToolCapability::EvidenceSynthesis => {
                if request.evidence.is_empty() {
                    Some((
                        EvidenceState::Unmeasured,
                        "no provenance-bearing evidence record was supplied".to_string(),
                    ))
                } else if request
                    .evidence
                    .iter()
                    .all(|record| !record.tier.is_verified())
                {
                    Some((
                        EvidenceState::Uninterpretable,
                        "all supplied evidence is marked unverified".to_string(),
                    ))
                } else {
                    None
                }
            }
            ToolCapability::MolecularContext => request
                .glioma_molecular
                .as_ref()
                .and_then(molecular_panel_gap)
                .or_else(|| observation_gap(request, ToolCapability::MolecularContext)),
            ToolCapability::LongitudinalTrajectory => longitudinal_gap(request),
            other => observation_gap(request, *other),
        };
        if let Some((state, reason)) = state {
            gaps.push(EvidenceGap {
                capability: *capability,
                state,
                reason,
            });
        }
    }
    gaps
}

fn observation_gap(
    request: &CaseRequest,
    capability: ToolCapability,
) -> Option<(EvidenceState, String)> {
    let inputs = tool_spec(capability).required_inputs;
    let mut missing = Vec::new();
    let mut bad_state = None;
    for required_kind in inputs {
        // A complete typed molecular panel is the structured realization of the
        // molecular observation. Keep the independent histology requirement,
        // but do not force callers to duplicate the same assay inventory in the
        // legacy free-text observation list.
        if capability == ToolCapability::MolecularContext
            && required_kind == ObservationKind::Molecular
            && request
                .glioma_molecular
                .as_ref()
                .is_some_and(|panel| molecular_panel_gap(panel).is_none())
        {
            continue;
        }
        match request
            .observations
            .iter()
            .filter(|observation| observation.kind == required_kind)
            .map(|observation| observation.status)
            .collect::<Vec<_>>()
            .as_slice()
        {
            [] => missing.push(format!("{required_kind:?}")),
            statuses => {
                if statuses.contains(&ObservationStatus::Conflicting) {
                    bad_state = Some(EvidenceState::Conflicting);
                } else if statuses.contains(&ObservationStatus::Uninterpretable)
                    && !statuses.contains(&ObservationStatus::Observed)
                {
                    bad_state = Some(EvidenceState::Uninterpretable);
                } else if !statuses.contains(&ObservationStatus::Observed) {
                    missing.push(format!("{required_kind:?}"));
                }
            }
        }
    }
    if let Some(state) = bad_state {
        Some((
            state,
            "one or more required observations are not interpretable".to_string(),
        ))
    } else if !missing.is_empty() {
        Some((
            EvidenceState::Unmeasured,
            format!(
                "required observation kind(s) missing or not collected: {}",
                missing.join(", ")
            ),
        ))
    } else {
        None
    }
}

fn molecular_panel_gap(panel: &GliomaMolecularPanel) -> Option<(EvidenceState, String)> {
    let coverage = panel.coverage();
    let gap = coverage.gaps.first()?;
    let state = match gap.state {
        GliomaEvidenceState::Present | GliomaEvidenceState::Absent => EvidenceState::Unmeasured,
        GliomaEvidenceState::NotCollected => EvidenceState::Unmeasured,
        GliomaEvidenceState::Uninterpretable => EvidenceState::Uninterpretable,
        GliomaEvidenceState::Conflicting => EvidenceState::Conflicting,
    };
    Some((state, gap.reason.clone()))
}

fn longitudinal_gap(request: &CaseRequest) -> Option<(EvidenceState, String)> {
    let temporal = audit_temporal(request).ok()?;
    if temporal.input_order_inversion_count > 0 {
        return Some((
            EvidenceState::Conflicting,
            format!(
                "{} caller-order timestamp inversion(s) require temporal review before alignment",
                temporal.input_order_inversion_count
            ),
        ));
    }
    let coverage = temporal
        .kind_coverage
        .iter()
        .find(|coverage| coverage.observation_kind == ObservationKind::LongitudinalOutcome)?;
    match coverage.state {
        crate::TemporalCoverageState::Complete => None,
        crate::TemporalCoverageState::Partial => Some((
            EvidenceState::Unmeasured,
            "longitudinal outcome records are only partially timestamped".to_string(),
        )),
        crate::TemporalCoverageState::Missing | crate::TemporalCoverageState::NotObserved => {
            Some((
                EvidenceState::Unmeasured,
                "longitudinal outcome requires at least one caller-supplied observed_at timestamp"
                    .to_string(),
            ))
        }
    }
}

fn run_tool(capability: ToolCapability, request: &CaseRequest, gaps: &[EvidenceGap]) -> ToolRun {
    let gap = gaps.iter().find(|gap| gap.capability == capability);
    let (status, findings) = match capability {
        ToolCapability::SafetyGate => (
            ToolRunStatus::Completed,
            vec![finding(
                "research_boundary_active",
                "request passed the research/education boundary; no clinical action is represented",
                vec![],
            )],
        ),
        ToolCapability::CaseIntegrity => {
            let missing_provenance = request
                .observations
                .iter()
                .filter(|observation| observation.source_id.is_none())
                .count();
            if request.observations.is_empty() {
                (
                    ToolRunStatus::NeedsInput,
                    vec![finding(
                        "no_observations",
                        "no observations were supplied; provenance was not assessed",
                        vec![],
                    )],
                )
            } else if missing_provenance == 0 {
                (
                    ToolRunStatus::Completed,
                    vec![finding(
                        "provenance_present",
                        "all observations carry caller-supplied source identifiers",
                        request
                            .observations
                            .iter()
                            .filter_map(|observation| observation.source_id.clone())
                            .collect(),
                    )],
                )
            } else {
                (
                    ToolRunStatus::NeedsInput,
                    vec![finding(
                        "provenance_gap",
                        &format!("{missing_provenance} observation(s) lack source_id; no provenance claim is made"),
                        vec![],
                    )],
                )
            }
        }
        ToolCapability::EvidenceGapScan => {
            let findings = gaps
                .iter()
                .map(|gap| {
                    finding(
                        "evidence_gap",
                        &format!("{}: {}", gap.capability.slug(), gap.reason),
                        vec![],
                    )
                })
                .collect::<Vec<_>>();
            (
                if findings.is_empty() {
                    ToolRunStatus::Completed
                } else {
                    ToolRunStatus::NeedsInput
                },
                findings,
            )
        }
        ToolCapability::HumanReviewHold => (
            ToolRunStatus::HeldForHumanReview,
            vec![finding(
                "human_review_required",
                "a qualified human must review the report before downstream use",
                vec![],
            )],
        ),
        ToolCapability::MolecularContext => {
            if let Some(panel) = &request.glioma_molecular {
                let coverage = panel.coverage();
                let mut findings = vec![finding(
                    "molecular_panel_inventory",
                    &format!(
                        "typed glioma panel covers {} of {} markers with {} measured call(s) ({} provenance-complete), {} assay(s), and {} specimen label(s); no classification is attempted",
                        coverage.marker_count.saturating_sub(coverage.not_collected_count),
                        coverage.marker_count,
                        coverage.measured_count,
                        coverage.provenance_complete_count,
                        coverage.assay_count,
                        coverage.specimen_count,
                    ),
                    coverage.source_ids.clone(),
                )];
                if !coverage.gaps.is_empty() {
                    findings.push(finding(
                        "molecular_panel_gap",
                        &coverage
                            .gaps
                            .iter()
                            .map(|gap| gap.reason.clone())
                            .collect::<Vec<_>>()
                            .join("; "),
                        vec![],
                    ));
                }
                (
                    if gap.is_some() {
                        ToolRunStatus::NeedsInput
                    } else {
                        ToolRunStatus::Completed
                    },
                    findings,
                )
            } else {
                (
                    if gap.is_some() {
                        ToolRunStatus::NeedsInput
                    } else {
                        ToolRunStatus::Completed
                    },
                    vec![finding(
                        if gap.is_some() {
                            "input_gap"
                        } else {
                            "input_inventory"
                        },
                        "Histology, Molecular; this tool organizes inputs and does not infer a clinical conclusion",
                        vec![],
                    )],
                )
            }
        }
        ToolCapability::EvidenceSynthesis => {
            let ids = request
                .evidence
                .iter()
                .map(|record| record.id.clone())
                .collect();
            (
                if gap.is_some() {
                    ToolRunStatus::NeedsInput
                } else {
                    ToolRunStatus::Completed
                },
                vec![finding(
                    "evidence_inventory",
                    &format!("inventoried {} caller-supplied evidence record(s); applicability is not asserted", request.evidence.len()),
                    ids,
                )],
            )
        }
        ToolCapability::LongitudinalTrajectory => {
            let temporal = audit_temporal(request).ok();
            let mut findings = Vec::new();
            if let Some(temporal) = temporal {
                findings.push(finding(
                    "temporal_alignment_inventory",
                    &format!(
                        "temporal audit found {} timestamped observation(s) across {} exact timepoint(s), {} undated observation(s), and status {:?}; no interval interpretation is attempted",
                        temporal.timestamped_observation_count,
                        temporal.distinct_timestamp_count,
                        temporal.untimestamped_observation_count,
                        temporal.status,
                    ),
                    temporal
                        .observations
                        .iter()
                        .filter_map(|observation| observation.source_id.clone())
                        .collect(),
                ));
                for temporal_finding in temporal.findings.iter().take(4) {
                    findings.push(finding(
                        "temporal_review_finding",
                        &format!("{}: {}", temporal_finding.code, temporal_finding.detail),
                        Vec::new(),
                    ));
                }
            } else {
                findings.push(finding(
                    "temporal_alignment_unavailable",
                    "temporal metadata could not be projected; no trajectory claim is made",
                    Vec::new(),
                ));
            }
            (
                if gap.is_some() {
                    ToolRunStatus::NeedsInput
                } else {
                    ToolRunStatus::Completed
                },
                findings,
            )
        }
        ToolCapability::RealDataInventory => (
            if gap.is_some() {
                ToolRunStatus::NeedsInput
            } else {
                ToolRunStatus::Completed
            },
            if gap.is_some() {
                vec![finding(
                    "real_data_bundle_required",
                    "a validated public glioma bundle is required; population records are never treated as patient findings",
                    vec![],
                )]
            } else {
                // A validated bundle adds the detailed inventory finding in
                // `annotate_real_data_tool_run`; avoid emitting an empty generic input record.
                Vec::new()
            },
        ),
        ToolCapability::RealDataQuery => (
            if gap.is_some() {
                ToolRunStatus::NeedsInput
            } else {
                ToolRunStatus::Completed
            },
            if gap.is_some() {
                vec![finding(
                    "real_data_query_required",
                    "an explicit real_data_query is required; no population record is selected implicitly",
                    vec![],
                )]
            } else {
                Vec::new()
            },
        ),
        _ => {
            let spec = tool_spec(capability);
            let input_labels = spec
                .required_inputs
                .iter()
                .map(|kind| format!("{kind:?}"))
                .collect::<Vec<_>>();
            (
                if gap.is_some() {
                    ToolRunStatus::NeedsInput
                } else {
                    ToolRunStatus::Completed
                },
                vec![finding(
                    if gap.is_some() {
                        "input_gap"
                    } else {
                        "input_inventory"
                    },
                    &format!(
                        "{}; this tool organizes inputs and does not infer a clinical conclusion",
                        input_labels.join(", ")
                    ),
                    vec![],
                )],
            )
        }
    };
    ToolRun {
        capability,
        status,
        findings,
    }
}

fn finding(code: &str, detail: &str, evidence_ids: Vec<String>) -> ToolFinding {
    ToolFinding {
        code: code.to_string(),
        detail: detail.to_string(),
        evidence_ids,
    }
}

fn annotate_real_data_tool_run(
    run: &mut ToolRun,
    summary: &RealDataSummary,
    data: &RealGliomaBundle,
    request: &CaseRequest,
) -> Result<(), NeurosurgeryError> {
    let genomic_project_distribution = summary
        .genomic_project_case_counts
        .iter()
        .map(|entry| format!("{}={}", entry.project_id, entry.case_count))
        .collect::<Vec<_>>();
    let genomic_project_distribution = if genomic_project_distribution.is_empty() {
        "not reported".to_string()
    } else {
        genomic_project_distribution.join(", ")
    };
    let genomic_data_type_distribution = summary
        .genomic_project_data_type_counts
        .iter()
        .map(|entry| {
            format!(
                "{}:{}={}",
                entry.project_id, entry.data_type, entry.file_count
            )
        })
        .collect::<Vec<_>>();
    let genomic_data_type_distribution = if genomic_data_type_distribution.is_empty() {
        "not reported".to_string()
    } else {
        genomic_data_type_distribution.join(", ")
    };
    if run.capability == ToolCapability::RealDataInventory {
        let profile_modalities = summary
            .portal_profile_type_counts
            .iter()
            .map(|entry| format!("{}={}", entry.alteration_type, entry.count))
            .collect::<Vec<_>>();
        let profile_modalities = if profile_modalities.is_empty() {
            "not reported".to_string()
        } else {
            profile_modalities.join(", ")
        };
        let status_distribution = summary
            .trial_status_counts
            .iter()
            .map(|entry| format!("{}={}", entry.status, entry.count))
            .collect::<Vec<_>>();
        let status_distribution = if status_distribution.is_empty() {
            "not reported".to_string()
        } else {
            status_distribution.join(", ")
        };
        let latest_trial_update = summary
            .latest_trial_update
            .as_deref()
            .unwrap_or("not reported");
        run.findings.push(finding(
            "real_data_inventory",
            &format!(
                "inventory contains {} registry record(s) ({} recruiting, {} completed; status distribution: {}; latest registry update: {}); {} trial(s) expose study type, {} aggregate enrollment target(s), and {} intervention list(s); {} genomic project(s) covering {} aggregate case(s) (per project: {}), with GDC file/data-type facets [{}]; {} public portal study/studies covering {} published sample(s) with {} molecular-profile metadata record(s) (modalities: {}), {} explicit study/profile/publication relationship(s), {} PMID-linked study record(s), and {} indexed PubMed citation(s) ({} with abstract text); PMID crosswalk links {} portal study/study record(s), leaves {} portal PMID(s) unmatched, {} portal study record(s) without a PMID, and {} citation(s) without a portal study; bundle digest {}",
                summary.clinical_trial_count,
                summary.recruiting_trial_count,
                summary.completed_trial_count,
                status_distribution,
                latest_trial_update,
                summary.trial_study_type_count,
                summary.trial_enrollment_count,
                summary.trial_intervention_count,
                summary.genomic_project_count,
                summary.genomic_case_count,
                genomic_project_distribution,
                genomic_data_type_distribution,
                summary.portal_study_count,
                summary.portal_sample_count,
                summary.portal_molecular_profile_count,
                profile_modalities,
                summary.relationship_count,
                summary.public_pmid_count,
                summary.literature_article_count,
                summary.literature_abstract_count,
                summary.portal_literature_linked_count,
                summary.portal_literature_unlinked_count,
                summary.portal_study_without_pmid_count,
                summary.literature_without_portal_count,
                summary.bundle_digest
            ),
            data.sources
                .iter()
                .map(|source| source.source_id.clone())
                .collect(),
        ));
    } else if run.capability == ToolCapability::RealDataQuery {
        let query = request.real_data_query.as_ref().ok_or_else(|| {
            NeurosurgeryError::RealDataRejected {
                reason: "real_data_query route has no query object".to_string(),
            }
        })?;
        let result = data.query(query)?;
        let hit_summary = result
            .hits
            .iter()
            .map(|hit| format!("{}={}", hit.record_kind.slug(), hit.record_id))
            .collect::<Vec<_>>();
        run.findings.push(finding(
            "real_data_query",
            &format!(
                "matched {} public record(s), returned {} bounded hit(s), truncated={}; {} (bundle digest {})",
                result.total_matches,
                result.returned_matches,
                result.truncated,
                if hit_summary.is_empty() {
                    "no matching records".to_string()
                } else {
                    hit_summary.join(", ")
                },
                summary.bundle_digest
            ),
            result
                .hits
                .iter()
                .map(|hit| hit.source_id.clone())
                .collect(),
        ));
    } else if run.capability == ToolCapability::EvidenceSynthesis {
        let profile_modalities = summary
            .portal_profile_type_counts
            .iter()
            .map(|entry| format!("{}={}", entry.alteration_type, entry.count))
            .collect::<Vec<_>>();
        let profile_modalities = if profile_modalities.is_empty() {
            "not reported".to_string()
        } else {
            profile_modalities.join(", ")
        };
        let status_distribution = summary
            .trial_status_counts
            .iter()
            .map(|entry| format!("{}={}", entry.status, entry.count))
            .collect::<Vec<_>>();
        let status_distribution = if status_distribution.is_empty() {
            "not reported".to_string()
        } else {
            status_distribution.join(", ")
        };
        let latest_trial_update = summary
            .latest_trial_update
            .as_deref()
            .unwrap_or("not reported");
        run.findings.push(finding(
            "real_data_provenance",
            &format!(
                "validated {} public source(s) and {} real record(s): {} recruiting trial(s), {} completed trial(s), status distribution [{}], latest registry update {}; {} trial(s) expose study type, {} aggregate enrollment target(s), and {} intervention list(s); {} aggregate genomic case(s) (per project: {}), with GDC file/data-type facets [{}]; {} published portal sample(s) and {} molecular-profile metadata record(s) (modalities: {}), {} explicit study/profile/publication relationship(s), {} indexed PubMed citation(s) ({} with abstract text); PMID crosswalk links {} portal study/study record(s), with {} unmatched portal PMID(s), {} portal study record(s) without a PMID, and {} citation(s) without a portal study; bundle digest {}",
                summary.source_count,
                summary.record_count,
                summary.recruiting_trial_count,
                summary.completed_trial_count,
                status_distribution,
                latest_trial_update,
                summary.trial_study_type_count,
                summary.trial_enrollment_count,
                summary.trial_intervention_count,
                summary.genomic_case_count,
                genomic_project_distribution,
                genomic_data_type_distribution,
                summary.portal_sample_count,
                summary.portal_molecular_profile_count,
                profile_modalities,
                summary.relationship_count,
                summary.literature_article_count,
                summary.literature_abstract_count,
                summary.portal_literature_linked_count,
                summary.portal_literature_unlinked_count,
                summary.portal_study_without_pmid_count,
                summary.literature_without_portal_count,
                summary.bundle_digest
            ),
            data.sources
                .iter()
                .map(|source| source.source_id.clone())
                .collect(),
        ));
    }
    Ok(())
}

fn annotate_public_literature_tool_run(
    run: &mut ToolRun,
    summary: &crate::PublicLiteratureSummary,
    literature: &PublicLiteratureBundle,
) {
    if run.capability != ToolCapability::EvidenceSynthesis {
        return;
    }
    let coverage = summary
        .specialty_counts
        .iter()
        .map(|entry| format!("{}={}", entry.specialty.slug(), entry.count))
        .collect::<Vec<_>>();
    run.findings.push(finding(
        "public_literature_provenance",
        &format!(
            "validated {} public PubMed source(s) and {} citation record(s), {} with abstract text and {} explicitly truncated; specialty coverage [{}]; bundle digest {}; citation metadata remains unverified and requires human review",
            summary.source_count,
            summary.record_count,
            summary.abstract_count,
            summary.abstract_truncated_count,
            if coverage.is_empty() {
                "none".to_string()
            } else {
                coverage.join(", ")
            },
            summary.bundle_digest
        ),
        literature
            .sources
            .iter()
            .map(|source| source.source_id.clone())
            .collect(),
    ));
}

fn research_hypotheses(specialty: Specialty) -> Vec<ResearchHypothesis> {
    let checks = |items: &[&str]| items.iter().map(|item| (*item).to_string()).collect();
    match specialty {
        Specialty::Glioma => vec![ResearchHypothesis {
            label: "integrated tumour-identity and post-treatment-change hypotheses".to_string(),
            status: "research hypothesis set; not a diagnosis".to_string(),
            discriminating_checks: checks(&[
                "align histology, molecular calls, imaging context, and acquisition time",
                "separate unrun assays from negative findings",
                "compare competing explanations for interval change with provenance",
            ]),
        }],
        Specialty::CranialBase => vec![ResearchHypothesis {
            label: "cranial-base compartment, interface, and corridor hypotheses".to_string(),
            status: "research hypothesis set; not an operative plan".to_string(),
            discriminating_checks: checks(&[
                "map supplied compartments and adjacent structures",
                "record imaging resolution and modality limitations",
                "route vascular, cranial-nerve, and functional questions to human review",
            ]),
        }],
        Specialty::Craniosynostosis => vec![ResearchHypothesis {
            label: "suture-pattern, craniofacial-growth, and developmental-trajectory hypotheses".to_string(),
            status: "research hypothesis set; not a treatment recommendation".to_string(),
            discriminating_checks: checks(&[
                "align developmental observations with imaging timepoints",
                "keep syndromic and non-syndromic labels as hypotheses until evidence is integrated",
                "identify missing functional and genetic context",
            ]),
        }],
        Specialty::Encephalocele => vec![ResearchHypothesis {
            label: "defect-anatomy, neural-content, and cerebrospinal-fluid-interface hypotheses".to_string(),
            status: "research hypothesis set; not a procedural recommendation".to_string(),
            discriminating_checks: checks(&[
                "map defect location and supplied neural or vascular relationships",
                "separate imaging description from inferred tissue identity",
                "record developmental and functional evidence gaps",
            ]),
        }],
        Specialty::SpinaBifida => vec![ResearchHypothesis {
            label: "spinal-dysraphism, neural-function, and longitudinal-trajectory hypotheses".to_string(),
            status: "research hypothesis set; not an intervention plan".to_string(),
            discriminating_checks: checks(&[
                "align spinal anatomy with neurologic-function observations",
                "distinguish absent assessment from normal function",
                "surface longitudinal change and source conflicts",
            ]),
        }],
        Specialty::ChiariMalformation => vec![ResearchHypothesis {
            label: "craniocervical-junction geometry, associated-findings, and trajectory hypotheses".to_string(),
            status: "research hypothesis set; not a diagnosis or decompression recommendation".to_string(),
            discriminating_checks: checks(&[
                "inventory supplied junction imaging and measurement context",
                "separate associated findings from unassessed possibilities",
                "compare symptoms and function only when caller-supplied and time-aligned",
            ]),
        }],
    }
}

fn build_report(
    request: &CaseRequest,
    gaps: &[EvidenceGap],
    molecular_summary: Option<&GliomaMolecularSummary>,
) -> ResearchReport {
    let known_inputs = request
        .observations
        .iter()
        .filter(|observation| observation.status == ObservationStatus::Observed)
        .map(|observation| format!("{}: {}", observation.kind.slug(), observation.label))
        .chain(
            request
                .evidence
                .iter()
                .filter(|record| record.tier.is_verified())
                .map(|record| format!("evidence {}: {}", record.id, record.title)),
        )
        .chain(molecular_summary.into_iter().map(|summary| {
            format!(
                "typed glioma molecular panel: {} measured call(s) ({} provenance-complete) across {} marker(s), digest {}",
                summary.measured_count,
                summary.provenance_complete_count,
                summary.marker_count,
                summary.panel_digest
            )
        }))
        .collect::<Vec<_>>();
    let mut uncertainties = request
        .observations
        .iter()
        .filter(|observation| observation.status != ObservationStatus::Observed)
        .map(|observation| {
            format!(
                "{} observation {:?} is {:?}; it is not treated as a negative finding",
                observation.label, observation.kind, observation.status
            )
        })
        .collect::<Vec<_>>();
    uncertainties.extend(
        gaps.iter()
            .map(|gap| format!("{}: {}", gap.capability.label(), gap.reason)),
    );
    if let Some(summary) = molecular_summary {
        uncertainties.extend(
            summary
                .research_gaps
                .iter()
                .map(|gap| format!("glioma molecular panel: {gap}")),
        );
    }
    let next_research_questions = if gaps.is_empty() {
        vec![
            "Which claims remain bounded to the supplied population and modality?".to_string(),
            "Which independent reviewer will adjudicate conflicts before downstream use?"
                .to_string(),
        ]
    } else {
        gaps.iter()
            .map(|gap| format!("Collect or verify inputs for {} without treating absence as a negative finding.", gap.capability.label()))
            .collect()
    };
    let research_worklist = build_research_worklist(request.specialty, gaps);
    ResearchReport {
        non_clinical_use_notice: "Research and education support only. This report does not diagnose, prognosticate, triage, recommend treatment, or direct a procedure.".to_string(),
        scope: format!("{}; request use is {:?}; all tools are read-only", request.specialty.display_name(), request.request_use),
        observed_finding_count: request
            .observations
            .iter()
            .filter(|observation| observation.status == ObservationStatus::Observed)
            .count(),
        evidence_record_count: request.evidence.len(),
        known_inputs,
        uncertainties,
        next_research_questions,
        research_worklist,
        prohibited_actions: vec![
            "assigning an individual diagnosis".to_string(),
            "predicting an individual's outcome".to_string(),
            "selecting or recommending therapy".to_string(),
            "triaging or issuing an urgent clinical alert".to_string(),
            "generating an operative or invasive procedural plan".to_string(),
        ],
    }
}

fn build_research_worklist(specialty: Specialty, gaps: &[EvidenceGap]) -> Vec<ResearchWorkItem> {
    let reviewer_roles = specialty.profile().human_review_roles;
    gaps.iter()
        .enumerate()
        .map(|(index, gap)| {
            let status = match gap.state {
                EvidenceState::Unmeasured => ResearchWorkItemStatus::NeedsCallerEvidence,
                EvidenceState::Uninterpretable | EvidenceState::Conflicting => {
                    ResearchWorkItemStatus::NeedsHumanReview
                }
                EvidenceState::Measured => ResearchWorkItemStatus::NeedsHumanReview,
            };
            let spec = tool_spec(gap.capability);
            ResearchWorkItem {
                sequence: (index + 1) as u16,
                capability: gap.capability,
                status,
                evidence_state: gap.state,
                objective: spec.purpose,
                reason: gap.reason.clone(),
                required_observations: spec.required_inputs,
                reviewer_roles: reviewer_roles.clone(),
            }
        })
        .collect()
}

trait ObservationKindSlug {
    fn slug(self) -> &'static str;
}

impl ObservationKindSlug for ObservationKind {
    fn slug(self) -> &'static str {
        match self {
            ObservationKind::Imaging => "imaging",
            ObservationKind::Histology => "histology",
            ObservationKind::Molecular => "molecular",
            ObservationKind::Neuroanatomy => "neuroanatomy",
            ObservationKind::NeurologicFunction => "neurologic_function",
            ObservationKind::DevelopmentalTrajectory => "developmental_trajectory",
            ObservationKind::SpinalDysraphism => "spinal_dysraphism",
            ObservationKind::CraniocervicalJunction => "craniocervical_junction",
            ObservationKind::SurgicalHistory => "surgical_history",
            ObservationKind::LongitudinalOutcome => "longitudinal_outcome",
        }
    }
}
