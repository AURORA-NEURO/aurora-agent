//! MCP Neurosurgery intake and evidence handlers.
//!
//! Kept beside the dispatcher so each domain surface can be reviewed independently.

use super::*;

impl Server {
    /// Run the local, read-only neurosurgical research route. The request is accepted either as
    /// the tool arguments themselves or under a `request` key, so the MCP shape can stay stable
    /// when a caller wraps this tool in a larger mission. No provider, network, credential, or
    /// clinical effect is reachable from this handler.
    pub(super) fn neurosurgery_plan(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .cloned()
            .unwrap_or_else(|| arguments.clone());
        let agent = NeurosurgicalAgent::default();
        if arguments.get("real_glioma_data").is_some()
            && arguments.get("public_literature").is_some()
        {
            return Err(
                "neurosurgery_plan accepts one evidence bundle: choose real_glioma_data or public_literature"
                    .to_string(),
            );
        }
        let response = if let Some(real_data) = arguments.get("real_glioma_data") {
            agent
                .run_json_with_real_glioma_data(&request, real_data)
                .map_err(|error| format!("neurosurgical research route refused: {error}"))
        } else if let Some(public_literature) = arguments.get("public_literature") {
            let request: CaseRequest = serde_json::from_value(request)
                .map_err(|error| format!("invalid neurosurgical research request: {error}"))?;
            let literature: PublicLiteratureBundle =
                serde_json::from_value(public_literature.clone())
                    .map_err(|error| format!("invalid public-literature bundle: {error}"))?;
            agent
                .run_with_public_literature(&request, &literature)
                .map_err(|error| format!("public-literature neurosurgical route refused: {error}"))
        } else {
            agent
                .run_json(&request)
                .map_err(|error| format!("neurosurgical research route refused: {error}"))
        }
        .map_err(|error| format!("neurosurgical research route refused: {error}"))?;
        serde_json::to_value(response)
            .map_err(|error| format!("cannot encode neurosurgical research response: {error}"))
    }

    /// Route a bounded natural-language question into the closed neurosurgical specialty/tool
    /// catalogue. This is lexical, abstaining, provider-free intake; it does not construct a
    /// clinical case or execute any downstream tool.
    pub(super) fn neurosurgery_intake_plan(&self, arguments: &Value) -> Result<Value, String> {
        let query: NeurosurgicalIntakeQuery = serde_json::from_value(arguments.clone())
            .map_err(|error| format!("invalid neurosurgical intake query: {error}"))?;
        let report = NeurosurgicalAgent::default()
            .intake_plan(&query)
            .map_err(|error| format!("neurosurgical intake planning refused: {error}"))?;
        serde_json::to_value(report)
            .map_err(|error| format!("cannot encode neurosurgical intake plan: {error}"))
    }

    /// Compose bounded natural-language intake with a digest-only research mission. The
    /// orchestrator refuses absent or mismatched public evidence before any mission executes and
    /// can carry a validated metadata-only real case-asset manifest or sanitized DICOM/FHIR
    /// metadata imports into the nested mission.
    pub(super) fn neurosurgery_intake_mission(&self, arguments: &Value) -> Result<Value, String> {
        let query: NeurosurgicalIntakeQuery = serde_json::from_value(arguments.clone())
            .map_err(|error| format!("invalid neurosurgical intake query: {error}"))?;
        let real_data = arguments
            .get("real_glioma_data")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<RealGliomaBundle>(value.clone())
                    .map_err(|error| format!("invalid real-glioma bundle: {error}"))
            })
            .transpose()?;
        let public_literature = arguments
            .get("public_literature")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<PublicLiteratureBundle>(value.clone())
                    .map_err(|error| format!("invalid public-literature bundle: {error}"))
            })
            .transpose()?;
        let case_asset_manifest = arguments
            .get("case_asset_manifest")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<CaseAssetManifest>(value.clone())
                    .map_err(|error| format!("invalid case-asset manifest: {error}"))
            })
            .transpose()?;
        let case_asset_query = arguments
            .get("case_asset_manifest_query")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<CaseAssetManifestQuery>(value.clone())
                    .map_err(|error| format!("invalid case-asset manifest query: {error}"))
            })
            .transpose()?;
        let case_asset_disposition = arguments
            .get("case_asset_review_disposition")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<CaseAssetReviewDispositionReport>(value.clone()).map_err(
                    |error| format!("invalid intake case-asset review disposition: {error}"),
                )
            })
            .transpose()?;
        let case_dicom_import = arguments
            .get("case_dicom_import")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<DicomCaseImport>(value.clone())
                    .map_err(|error| format!("invalid intake DICOM import: {error}"))
            })
            .transpose()?;
        let case_fhir_import = arguments
            .get("case_fhir_import")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<FhirCaseImport>(value.clone())
                    .map_err(|error| format!("invalid intake FHIR import: {error}"))
            })
            .transpose()?;
        if case_asset_query.is_some() && case_asset_manifest.is_none() {
            return Err(
                "neurosurgery_intake_mission case_asset_manifest_query requires case_asset_manifest"
                .to_string(),
            );
        }
        if (case_dicom_import.is_some() || case_fhir_import.is_some())
            && case_asset_manifest.is_some()
        {
            return Err(
                "neurosurgery_intake_mission DICOM/FHIR imports cannot be combined with case_asset_manifest"
                .to_string(),
            );
        }
        if case_asset_disposition.is_some()
            && case_asset_manifest.is_none()
            && case_dicom_import.is_none()
            && case_fhir_import.is_none()
        {
            return Err(
                "neurosurgery_intake_mission case_asset_review_disposition requires a case asset manifest or DICOM/FHIR import"
                    .to_string(),
            );
        }
        let freshness = arguments
            .get("freshness")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<RealDataFreshnessQuery>(value.clone()).map_err(|error| {
                    format!("invalid neurosurgical intake freshness query: {error}")
                })
            })
            .transpose()?;
        let max_steps = arguments
            .get("max_session_steps")
            .and_then(Value::as_u64)
            .map(|value| {
                usize::try_from(value)
                    .map_err(|_| "max_session_steps exceeds platform limits".to_string())
            })
            .transpose()?
            .unwrap_or(MAX_SESSION_STEPS);
        let report = if case_dicom_import.is_some() || case_fhir_import.is_some() {
            NeurosurgicalAgent::default().run_intake_mission_with_case_imports_and_dispositions(
                &query,
                real_data.as_ref(),
                public_literature.as_ref(),
                case_dicom_import.as_ref(),
                case_fhir_import.as_ref(),
                freshness.as_ref(),
                case_asset_disposition.as_ref(),
                max_steps,
            )
        } else {
            NeurosurgicalAgent::default().run_intake_mission_with_case_assets_and_dispositions(
                &query,
                real_data.as_ref(),
                public_literature.as_ref(),
                case_asset_manifest.as_ref(),
                case_asset_query.as_ref(),
                freshness.as_ref(),
                case_asset_disposition.as_ref(),
                max_steps,
            )
        };
        let report =
            report.map_err(|error| format!("neurosurgical intake mission refused: {error}"))?;
        serde_json::to_value(report)
            .map_err(|error| format!("cannot encode neurosurgical intake mission: {error}"))
    }

    /// Fan out a bounded intake question across one or all six validated public-literature lanes.
    pub(super) fn neurosurgery_intake_portfolio(&self, arguments: &Value) -> Result<Value, String> {
        let query: NeurosurgicalIntakePortfolioQuery = serde_json::from_value(arguments.clone())
            .map_err(|error| format!("invalid neurosurgical intake portfolio query: {error}"))?;
        let real_data = arguments
            .get("real_glioma_data")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<RealGliomaBundle>(value.clone())
                    .map_err(|error| format!("invalid real-glioma bundle: {error}"))
            })
            .transpose()?;
        let public_literature = arguments
            .get("public_literature")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<PublicLiteratureBundle>(value.clone())
                    .map_err(|error| format!("invalid public-literature bundle: {error}"))
            })
            .transpose()?;
        let case_asset_manifest = arguments
            .get("case_asset_manifest")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<CaseAssetManifest>(value.clone())
                    .map_err(|error| format!("invalid case-asset manifest: {error}"))
            })
            .transpose()?;
        let case_asset_query = arguments
            .get("case_asset_manifest_query")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<CaseAssetManifestQuery>(value.clone())
                    .map_err(|error| format!("invalid case-asset manifest query: {error}"))
            })
            .transpose()?;
        let case_asset_disposition = arguments
            .get("case_asset_review_disposition")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<CaseAssetReviewDispositionReport>(value.clone()).map_err(
                    |error| {
                        format!("invalid intake portfolio case-asset review disposition: {error}")
                    },
                )
            })
            .transpose()?;
        if case_asset_query.is_some() && case_asset_manifest.is_none() {
            return Err(
                "neurosurgery_intake_portfolio case_asset_manifest_query requires case_asset_manifest"
                    .to_string(),
            );
        }
        if case_asset_disposition.is_some() && case_asset_manifest.is_none() {
            return Err(
                "neurosurgery_intake_portfolio case_asset_review_disposition requires case_asset_manifest"
                    .to_string(),
            );
        }
        let freshness = arguments
            .get("freshness")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<RealDataFreshnessQuery>(value.clone()).map_err(|error| {
                    format!("invalid neurosurgical intake portfolio freshness query: {error}")
                })
            })
            .transpose()?;
        let report = NeurosurgicalAgent::default()
            .run_intake_portfolio_with_case_assets_and_freshness_and_dispositions(
                &query,
                real_data.as_ref(),
                public_literature.as_ref(),
                case_asset_manifest.as_ref(),
                case_asset_query.as_ref(),
                freshness.as_ref(),
                case_asset_disposition.as_ref(),
            )
            .map_err(|error| format!("neurosurgical intake portfolio refused: {error}"))?;
        serde_json::to_value(report)
            .map_err(|error| format!("cannot encode neurosurgical intake portfolio: {error}"))
    }

    /// Expose the closed specialty/profile/tool inventory without constructing a run.
    pub(super) fn neurosurgery_catalogue(&self, _arguments: &Value) -> Result<Value, String> {
        let agent = NeurosurgicalAgent::default();
        serde_json::to_value(json!({
            "schema_version": NEUROSURGERY_SCHEMA_VERSION,
            "specialties": agent.specialty_profiles(),
            "tools": agent.catalogue(),
            "standalone_tools": [
                "neurosurgery_intake_plan",
                "neurosurgery_intake_mission",
                "neurosurgery_intake_portfolio",
                "neurosurgery_evidence_audit",
                "neurosurgery_specialty_evidence_map",
                "neurosurgery_case_asset_manifest",
                "neurosurgery_case_asset_review_disposition",
                "neurosurgery_case_dicom_import",
                "neurosurgery_case_dicom_evidence_workflow",
                "neurosurgery_evidence_synthesis",
                "neurosurgery_glioma_molecular_map",
                "neurosurgery_evidence_graph",
                "neurosurgery_real_data_coverage",
                "neurosurgery_real_data_cohort_landscape",
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
                "neurosurgery_real_data_trial_landscape",
                "neurosurgery_real_data_molecular_coverage",
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
                "neurosurgery_evidence_program",
                "neurosurgery_public_literature_portfolio",
                "neurosurgery_research_brief",
                "neurosurgery_research_plan",
                "neurosurgery_evidence_acquisition",
            ],
            "provider": "none",
            "network": false,
            "effects": ["read_only"],
        }))
        .map_err(|error| format!("cannot encode neurosurgical catalogue: {error}"))
    }

    /// Audit specialty-specific observation coverage without interpreting the observations.
    pub(super) fn neurosurgery_evidence_audit(&self, arguments: &Value) -> Result<Value, String> {
        let request_value = arguments
            .get("request")
            .ok_or_else(|| "neurosurgery_evidence_audit requires request".to_string())?;
        let request: CaseRequest = serde_json::from_value(request_value.clone())
            .map_err(|error| format!("invalid neurosurgical audit request: {error}"))?;
        let report = NeurosurgicalAgent::default()
            .audit_evidence(&request)
            .map_err(|error| format!("neurosurgical evidence audit refused: {error}"))?;
        serde_json::to_value(report)
            .map_err(|error| format!("cannot encode neurosurgical evidence audit: {error}"))
    }

    /// Build a domain-specific identity/spatial/functional/temporal evidence map for a request.
    /// The map inventories caller metadata only and never interprets clinical content.
    pub(super) fn neurosurgery_specialty_evidence_map(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request_value = arguments
            .get("request")
            .ok_or_else(|| "neurosurgery_specialty_evidence_map requires request".to_string())?;
        let request: CaseRequest = serde_json::from_value(request_value.clone())
            .map_err(|error| format!("invalid neurosurgical specialty-map request: {error}"))?;
        let report = NeurosurgicalAgent::default()
            .specialty_evidence_map(&request)
            .map_err(|error| format!("neurosurgical specialty evidence map refused: {error}"))?;
        serde_json::to_value(report)
            .map_err(|error| format!("cannot encode neurosurgical specialty evidence map: {error}"))
    }

    /// Project a caller-owned real, de-identified multimodal asset manifest without opening or
    /// interpreting any asset bytes. Identifiers are returned only as digests for safe handoff.
    pub(super) fn neurosurgery_case_asset_manifest(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request_value = arguments
            .get("request")
            .ok_or_else(|| "neurosurgery_case_asset_manifest requires request".to_string())?;
        let request: CaseRequest = serde_json::from_value(request_value.clone())
            .map_err(|error| format!("invalid neurosurgical case-asset request: {error}"))?;
        let manifest_value = arguments.get("manifest").ok_or_else(|| {
            "neurosurgery_case_asset_manifest requires a de-identified manifest".to_string()
        })?;
        let manifest: CaseAssetManifest = serde_json::from_value(manifest_value.clone())
            .map_err(|error| format!("invalid neurosurgical case-asset manifest: {error}"))?;
        let query = arguments
            .get("query")
            .map(|value| {
                serde_json::from_value::<CaseAssetManifestQuery>(value.clone())
                    .map_err(|error| format!("invalid neurosurgical case-asset query: {error}"))
            })
            .transpose()?
            .unwrap_or_default();
        let report = NeurosurgicalAgent::default()
            .case_asset_manifest(&request, &manifest, &query)
            .map_err(|error| format!("neurosurgical case-asset manifest refused: {error}"))?;
        serde_json::to_value(report)
            .map_err(|error| format!("cannot encode neurosurgical case-asset report: {error}"))
    }

    /// Import a caller-sanitized FHIR Bundle as digest-only case-asset metadata. No references,
    /// codes, narratives, bytes, provider, API key, or network are accessed.
    pub(super) fn neurosurgery_case_fhir_import(&self, arguments: &Value) -> Result<Value, String> {
        let request_value = arguments
            .get("request")
            .ok_or_else(|| "neurosurgery_case_fhir_import requires request".to_string())?;
        let request: CaseRequest = serde_json::from_value(request_value.clone())
            .map_err(|error| format!("invalid neurosurgical FHIR import request: {error}"))?;
        let import_value = arguments.get("import").ok_or_else(|| {
            "neurosurgery_case_fhir_import requires a sanitized FHIR import object".to_string()
        })?;
        let import: FhirCaseImport = serde_json::from_value(import_value.clone())
            .map_err(|error| format!("invalid neurosurgical FHIR import: {error}"))?;
        let report = NeurosurgicalAgent::default()
            .case_fhir_import(&request, &import)
            .map_err(|error| format!("neurosurgical FHIR import refused: {error}"))?;
        serde_json::to_value(report)
            .map_err(|error| format!("cannot encode neurosurgical FHIR import report: {error}"))
    }

    /// Import standard DICOM JSON series metadata without opening pixel bytes or accepting
    /// patient-identifying tags. The returned inventory is digest-bound and human-review only.
    pub(super) fn neurosurgery_case_dicom_import(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request_value = arguments
            .get("request")
            .ok_or_else(|| "neurosurgery_case_dicom_import requires request".to_string())?;
        let request: CaseRequest = serde_json::from_value(request_value.clone())
            .map_err(|error| format!("invalid neurosurgical DICOM import request: {error}"))?;
        let import_value = arguments.get("import").ok_or_else(|| {
            "neurosurgery_case_dicom_import requires DICOM JSON metadata".to_string()
        })?;
        let import: DicomCaseImport = serde_json::from_value(import_value.clone())
            .map_err(|error| format!("invalid neurosurgical DICOM import: {error}"))?;
        let report = NeurosurgicalAgent::default()
            .case_dicom_import(&request, &import)
            .map_err(|error| format!("neurosurgical DICOM import refused: {error}"))?;
        serde_json::to_value(report)
            .map_err(|error| format!("cannot encode neurosurgical DICOM import report: {error}"))
    }

    /// Project DICOM metadata and bind it into the source-grounded evidence workers in one
    /// replayable, human-review-only envelope. Pixel bytes and clinical interpretation remain
    /// outside the MCP boundary.
    pub(super) fn neurosurgery_case_dicom_evidence_workflow(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request_value = arguments.get("request").ok_or_else(|| {
            "neurosurgery_case_dicom_evidence_workflow requires request".to_string()
        })?;
        let request: CaseRequest = serde_json::from_value(request_value.clone())
            .map_err(|error| format!("invalid neurosurgical DICOM workflow request: {error}"))?;
        let import_value = arguments.get("import").ok_or_else(|| {
            "neurosurgery_case_dicom_evidence_workflow requires DICOM JSON metadata".to_string()
        })?;
        let import: DicomCaseImport = serde_json::from_value(import_value.clone())
            .map_err(|error| format!("invalid neurosurgical DICOM workflow import: {error}"))?;
        let real_data = arguments
            .get("real_glioma_data")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<RealGliomaBundle>(value.clone())
                    .map_err(|error| format!("invalid real glioma bundle: {error}"))
            })
            .transpose()?;
        let public_literature = arguments
            .get("public_literature")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<PublicLiteratureBundle>(value.clone())
                    .map_err(|error| format!("invalid public literature bundle: {error}"))
            })
            .transpose()?;
        let query = arguments
            .get("query")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<DicomEvidenceWorkflowQuery>(value.clone())
                    .map_err(|error| format!("invalid DICOM evidence workflow query: {error}"))
            })
            .transpose()?
            .unwrap_or_default();
        let report = NeurosurgicalAgent::default()
            .case_dicom_evidence_workflow(
                &request,
                &import,
                real_data.as_ref(),
                public_literature.as_ref(),
                &query,
            )
            .map_err(|error| format!("neurosurgical DICOM evidence workflow refused: {error}"))?;
        serde_json::to_value(report).map_err(|error| {
            format!("cannot encode neurosurgical DICOM evidence workflow: {error}")
        })
    }

    /// Apply caller-owned, digest-bound dispositions to a persisted case-asset review projection.
    /// Sequence numbers address only returned review rows; omitted rows remain pending.
    pub(super) fn neurosurgery_case_asset_review_disposition(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let report_value = arguments.get("report").ok_or_else(|| {
            "neurosurgery_case_asset_review_disposition requires report".to_string()
        })?;
        let report = serde_json::from_value::<bioprism_neurosurgery::CaseAssetManifestReport>(
            report_value.clone(),
        )
        .map_err(|error| format!("invalid neurosurgical case-asset manifest report: {error}"))?;
        let decisions = arguments
            .get("decisions")
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new()));
        let decisions = serde_json::from_value::<Vec<CaseAssetReviewDecision>>(decisions)
            .map_err(|error| format!("invalid case-asset review decisions: {error}"))?;
        if decisions.len() > MAX_CASE_ASSET_REVIEW_DISPOSITIONS {
            return Err(format!(
                "decisions must contain at most {MAX_CASE_ASSET_REVIEW_DISPOSITIONS} items"
            ));
        }
        let result = NeurosurgicalAgent::default()
            .case_asset_review_disposition(&report, &decisions)
            .map_err(|error| {
                format!("neurosurgical case-asset review disposition refused: {error}")
            })?;
        serde_json::to_value(result).map_err(|error| {
            format!("cannot encode neurosurgical case-asset review disposition report: {error}")
        })
    }

    /// Align a de-identified case with validated public evidence planes without generating a
    /// clinical conclusion. Case text is redacted from the returned ledger; source bundles and
    /// an optional metadata-only asset manifest are revalidated before any local query runs.
    pub(super) fn neurosurgery_evidence_synthesis(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request_value = arguments
            .get("request")
            .ok_or_else(|| "neurosurgery_evidence_synthesis requires request".to_string())?;
        let request: CaseRequest = serde_json::from_value(request_value.clone())
            .map_err(|error| format!("invalid neurosurgical synthesis request: {error}"))?;
        let query = arguments
            .get("query")
            .map(|value| {
                serde_json::from_value::<EvidenceSynthesisQuery>(value.clone())
                    .map_err(|error| format!("invalid neurosurgical synthesis query: {error}"))
            })
            .transpose()?
            .unwrap_or_default();
        let real_data = arguments
            .get("real_glioma_data")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<RealGliomaBundle>(value.clone())
                    .map_err(|error| format!("invalid neurosurgical real glioma bundle: {error}"))
            })
            .transpose()?;
        let public_literature = arguments
            .get("public_literature")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<PublicLiteratureBundle>(value.clone()).map_err(|error| {
                    format!("invalid neurosurgical public-literature bundle: {error}")
                })
            })
            .transpose()?;
        let case_asset_manifest = arguments
            .get("case_asset_manifest")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<CaseAssetManifest>(value.clone()).map_err(|error| {
                    format!("invalid neurosurgical synthesis case asset manifest: {error}")
                })
            })
            .transpose()?;
        let case_asset_query = arguments
            .get("case_asset_manifest_query")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<CaseAssetManifestQuery>(value.clone()).map_err(|error| {
                    format!("invalid neurosurgical synthesis case asset query: {error}")
                })
            })
            .transpose()?;
        let case_asset_disposition = arguments
            .get("case_asset_review_disposition")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<CaseAssetReviewDispositionReport>(value.clone()).map_err(
                    |error| {
                        format!(
                            "invalid neurosurgical synthesis case asset review disposition: {error}"
                        )
                    },
                )
            })
            .transpose()?;
        if case_asset_query.is_some() && case_asset_manifest.is_none() {
            return Err(
                "neurosurgery_evidence_synthesis case_asset_manifest_query requires case_asset_manifest"
                    .to_string(),
            );
        }
        let agent = NeurosurgicalAgent::default();
        let case_asset_report = case_asset_manifest
            .as_ref()
            .map(|manifest| {
                agent
                    .case_asset_manifest(
                        &request,
                        manifest,
                        &case_asset_query.clone().unwrap_or_default(),
                    )
                    .map_err(|error| {
                        format!("neurosurgical synthesis asset projection refused: {error}")
                    })
            })
            .transpose()?;
        let report = agent
            .evidence_synthesis_with_case_assets_and_dispositions(
                &request,
                real_data.as_ref(),
                public_literature.as_ref(),
                &query,
                case_asset_report.as_ref(),
                case_asset_disposition.as_ref(),
            )
            .map_err(|error| format!("neurosurgical evidence synthesis refused: {error}"))?;
        serde_json::to_value(report).map_err(|error| {
            format!("cannot encode neurosurgical evidence synthesis report: {error}")
        })
    }

    /// Ground a typed glioma molecular panel against exact records in validated local snapshots.
    /// Marker matches are retrieval metadata only and cannot produce a clinical interpretation.
    pub(super) fn neurosurgery_glioma_molecular_map(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request_value = arguments
            .get("request")
            .ok_or_else(|| "neurosurgery_glioma_molecular_map requires request".to_string())?;
        let request: CaseRequest = serde_json::from_value(request_value.clone())
            .map_err(|error| format!("invalid glioma molecular-map request: {error}"))?;
        let query = arguments
            .get("query")
            .map(|value| {
                serde_json::from_value::<GliomaMolecularMapQuery>(value.clone())
                    .map_err(|error| format!("invalid glioma molecular-map query: {error}"))
            })
            .transpose()?
            .unwrap_or_default();
        let real_data = arguments
            .get("real_glioma_data")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<RealGliomaBundle>(value.clone())
                    .map_err(|error| format!("invalid glioma molecular-map real bundle: {error}"))
            })
            .transpose()?;
        let public_literature = arguments
            .get("public_literature")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<PublicLiteratureBundle>(value.clone())
                    .map_err(|error| format!("invalid glioma molecular-map public bundle: {error}"))
            })
            .transpose()?;
        let report = NeurosurgicalAgent::default()
            .glioma_molecular_map(
                &request,
                real_data.as_ref(),
                public_literature.as_ref(),
                &query,
            )
            .map_err(|error| format!("glioma molecular evidence map refused: {error}"))?;
        serde_json::to_value(report)
            .map_err(|error| format!("cannot encode glioma molecular evidence map report: {error}"))
    }

    /// Project only explicit study/profile/PMID crosswalks from a validated real glioma bundle.
    /// This is a bounded provenance view, not a biological knowledge graph or inference engine.
    pub(super) fn neurosurgery_evidence_graph(&self, arguments: &Value) -> Result<Value, String> {
        let data_value = arguments
            .get("real_glioma_data")
            .ok_or_else(|| "neurosurgery_evidence_graph requires real_glioma_data".to_string())?;
        let real_glioma_data: RealGliomaBundle = serde_json::from_value(data_value.clone())
            .map_err(|error| format!("invalid neurosurgical real glioma bundle: {error}"))?;
        let query = arguments
            .get("query")
            .map(|value| {
                serde_json::from_value::<EvidenceGraphQuery>(value.clone())
                    .map_err(|error| format!("invalid neurosurgical evidence-graph query: {error}"))
            })
            .transpose()?
            .unwrap_or_default();
        if query.max_nodes == 0 || query.max_nodes > MAX_EVIDENCE_GRAPH_NODES {
            return Err(format!(
                "query.max_nodes must be between 1 and {MAX_EVIDENCE_GRAPH_NODES}"
            ));
        }
        if query.max_edges == 0 || query.max_edges > MAX_EVIDENCE_GRAPH_EDGES {
            return Err(format!(
                "query.max_edges must be between 1 and {MAX_EVIDENCE_GRAPH_EDGES}"
            ));
        }
        let report = NeurosurgicalAgent::default()
            .evidence_graph(&real_glioma_data, &query)
            .map_err(|error| format!("neurosurgical evidence graph refused: {error}"))?;
        serde_json::to_value(report)
            .map_err(|error| format!("cannot encode neurosurgical evidence graph: {error}"))
    }
}
