//! Neurosurgery evidence, literature, and mission handlers.

use super::*;

impl Server {
    /// Compose a bounded cross-specialty PubMed handoff for a local model or human reviewer.
    pub(super) fn neurosurgery_public_literature_evidence_packet(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let data_value = arguments.get("public_literature").ok_or_else(|| {
            "neurosurgery_public_literature_evidence_packet requires public_literature".to_string()
        })?;
        let public_literature: PublicLiteratureBundle = serde_json::from_value(data_value.clone())
            .map_err(|error| format!("invalid neurosurgical public-literature bundle: {error}"))?;
        let query = arguments
            .get("query")
            .map(|value| {
                serde_json::from_value::<PublicLiteratureEvidencePacketQuery>(value.clone())
                    .map_err(|error| {
                        format!(
                            "invalid neurosurgical public-literature evidence-packet query: {error}"
                        )
                    })
            })
            .transpose()?
            .unwrap_or_default();
        let report = NeurosurgicalAgent::default()
            .public_literature_evidence_packet(&public_literature, &query)
            .map_err(|error| {
                format!("neurosurgical public-literature evidence packet refused: {error}")
            })?;
        serde_json::to_value(report).map_err(|error| {
            format!("cannot encode neurosurgical public-literature evidence packet report: {error}")
        })
    }

    /// Render one validated public-literature packet into bounded, source-addressable context for
    /// a caller-owned local model. The renderer never invokes a provider or interprets abstracts.
    pub(super) fn neurosurgery_public_literature_reasoning_context(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let data_value = arguments.get("public_literature").ok_or_else(|| {
            "neurosurgery_public_literature_reasoning_context requires public_literature"
                .to_string()
        })?;
        let public_literature: PublicLiteratureBundle = serde_json::from_value(data_value.clone())
            .map_err(|error| format!("invalid neurosurgical public-literature bundle: {error}"))?;
        let query = arguments
            .get("query")
            .map(|value| {
                serde_json::from_value::<PublicLiteratureReasoningContextQuery>(value.clone())
                    .map_err(|error| {
                        format!(
                            "invalid neurosurgical public-literature reasoning-context query: {error}"
                        )
                    })
            })
            .transpose()?
            .unwrap_or_default();
        let report = NeurosurgicalAgent::default()
            .public_literature_reasoning_context(&public_literature, &query)
            .map_err(|error| {
                format!("neurosurgical public-literature reasoning context refused: {error}")
            })?;
        serde_json::to_value(report).map_err(|error| {
            format!(
                "cannot encode neurosurgical public-literature reasoning context report: {error}"
            )
        })
    }

    /// Audit a cross-specialty local-model/reviewer draft against emitted PMID citations.
    pub(super) fn neurosurgery_public_literature_draft_audit(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let data_value = arguments.get("public_literature").ok_or_else(|| {
            "neurosurgery_public_literature_draft_audit requires public_literature".to_string()
        })?;
        let public_literature: PublicLiteratureBundle = serde_json::from_value(data_value.clone())
            .map_err(|error| format!("invalid neurosurgical public-literature bundle: {error}"))?;
        let query = arguments.get("query").cloned().unwrap_or_else(|| json!({}));
        let claims = arguments.get("claims").cloned().ok_or_else(|| {
            "neurosurgery_public_literature_draft_audit requires claims".to_string()
        })?;
        let request = serde_json::from_value::<PublicLiteratureDraftAuditRequest>(json!({
            "query": query,
            "claims": claims,
        }))
        .map_err(|error| format!("invalid neurosurgical public-literature draft: {error}"))?;
        let report = NeurosurgicalAgent::default()
            .public_literature_draft_audit(&public_literature, &request)
            .map_err(|error| {
                format!("neurosurgical public-literature draft audit refused: {error}")
            })?;
        serde_json::to_value(report).map_err(|error| {
            format!("cannot encode neurosurgical public-literature draft audit report: {error}")
        })
    }

    /// Fan out one bounded local-literature query across selected specialty lanes. Every lane
    /// retains its own packet and digest; this is corpus reconnaissance, not cross-specialty
    /// inference.
    pub(super) fn neurosurgery_public_literature_matrix(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let data_value = arguments.get("public_literature").ok_or_else(|| {
            "neurosurgery_public_literature_matrix requires public_literature".to_string()
        })?;
        let public_literature: PublicLiteratureBundle = serde_json::from_value(data_value.clone())
            .map_err(|error| format!("invalid neurosurgical public-literature bundle: {error}"))?;
        let query = arguments
            .get("query")
            .map(|value| {
                serde_json::from_value::<PublicLiteratureMatrixQuery>(value.clone()).map_err(
                    |error| {
                        format!("invalid neurosurgical public-literature matrix query: {error}")
                    },
                )
            })
            .transpose()?
            .unwrap_or_default();
        let report = NeurosurgicalAgent::default()
            .public_literature_matrix(&public_literature, &query)
            .map_err(|error| format!("neurosurgical public-literature matrix refused: {error}"))?;
        serde_json::to_value(report).map_err(|error| {
            format!("cannot encode neurosurgical public-literature matrix report: {error}")
        })
    }

    /// Report source-age posture for a validated cross-specialty PubMed bundle with an explicit
    /// caller-owned as-of timestamp.
    pub(super) fn neurosurgery_public_literature_freshness(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let data_value = arguments.get("public_literature").ok_or_else(|| {
            "neurosurgery_public_literature_freshness requires public_literature".to_string()
        })?;
        let public_literature: PublicLiteratureBundle = serde_json::from_value(data_value.clone())
            .map_err(|error| format!("invalid public-literature bundle: {error}"))?;
        let query_value = arguments
            .get("query")
            .ok_or_else(|| "neurosurgery_public_literature_freshness requires query".to_string())?;
        let query: RealDataFreshnessQuery = serde_json::from_value(query_value.clone())
            .map_err(|error| format!("invalid public-literature freshness query: {error}"))?;
        let report = NeurosurgicalAgent::default()
            .public_literature_freshness(&public_literature, &query)
            .map_err(|error| format!("public-literature freshness refused: {error}"))?;
        serde_json::to_value(report)
            .map_err(|error| format!("cannot encode public-literature freshness report: {error}"))
    }

    /// Reconcile two validated cross-specialty PubMed snapshots without fetching or promoting
    /// the candidate. The report retains source/PMID identity, lane coverage, and freshness facts.
    pub(super) fn neurosurgery_public_literature_refresh_audit(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let before_value = arguments.get("before_public_literature").ok_or_else(|| {
            "neurosurgery_public_literature_refresh_audit requires before_public_literature"
                .to_string()
        })?;
        let after_value = arguments.get("after_public_literature").ok_or_else(|| {
            "neurosurgery_public_literature_refresh_audit requires after_public_literature"
                .to_string()
        })?;
        let before: PublicLiteratureBundle = serde_json::from_value(before_value.clone())
            .map_err(|error| format!("invalid before public-literature bundle: {error}"))?;
        let after: PublicLiteratureBundle = serde_json::from_value(after_value.clone())
            .map_err(|error| format!("invalid after public-literature bundle: {error}"))?;
        let query = arguments
            .get("query")
            .map(|value| {
                serde_json::from_value::<PublicLiteratureRefreshAuditQuery>(value.clone()).map_err(
                    |error| format!("invalid public-literature refresh-audit query: {error}"),
                )
            })
            .transpose()?
            .unwrap_or_default();
        let report = NeurosurgicalAgent::default()
            .public_literature_refresh_audit(&before, &after, &query)
            .map_err(|error| format!("public-literature refresh audit refused: {error}"))?;
        serde_json::to_value(report).map_err(|error| {
            format!("cannot encode public-literature refresh audit report: {error}")
        })
    }

    /// Link a validated real glioma literature index to a public-literature lane by exact PMID/DOI
    /// identifiers only. The report never merges either source bundle.
    pub(super) fn neurosurgery_literature_link_audit(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let real_value = arguments.get("real_glioma_data").ok_or_else(|| {
            "neurosurgery_literature_link_audit requires real_glioma_data".to_string()
        })?;
        let public_value = arguments.get("public_literature").ok_or_else(|| {
            "neurosurgery_literature_link_audit requires public_literature".to_string()
        })?;
        let real_data: RealGliomaBundle = serde_json::from_value(real_value.clone())
            .map_err(|error| format!("invalid real glioma bundle: {error}"))?;
        let public_literature: PublicLiteratureBundle =
            serde_json::from_value(public_value.clone())
                .map_err(|error| format!("invalid public-literature bundle: {error}"))?;
        let query = arguments
            .get("query")
            .map(|value| {
                serde_json::from_value::<LiteratureLinkAuditQuery>(value.clone())
                    .map_err(|error| format!("invalid literature link-audit query: {error}"))
            })
            .transpose()?
            .unwrap_or_default();
        let report = NeurosurgicalAgent::default()
            .literature_link_audit(&real_data, &public_literature, &query)
            .map_err(|error| format!("literature link audit refused: {error}"))?;
        serde_json::to_value(report)
            .map_err(|error| format!("cannot encode literature link audit report: {error}"))
    }

    /// Audit source/record completeness and identifier hygiene in a validated public snapshot.
    pub(super) fn neurosurgery_public_literature_integrity_audit(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let public_value = arguments.get("public_literature").ok_or_else(|| {
            "neurosurgery_public_literature_integrity_audit requires public_literature".to_string()
        })?;
        let public_literature: PublicLiteratureBundle =
            serde_json::from_value(public_value.clone())
                .map_err(|error| format!("invalid public-literature bundle: {error}"))?;
        let query = arguments
            .get("query")
            .map(|value| {
                serde_json::from_value::<PublicLiteratureIntegrityAuditQuery>(value.clone())
                    .map_err(|error| {
                        format!("invalid public-literature integrity-audit query: {error}")
                    })
            })
            .transpose()?
            .unwrap_or_default();
        let report = NeurosurgicalAgent::default()
            .public_literature_integrity_audit(&public_literature, &query)
            .map_err(|error| format!("public-literature integrity audit refused: {error}"))?;
        serde_json::to_value(report).map_err(|error| {
            format!("cannot encode public-literature integrity audit report: {error}")
        })
    }

    /// Turn validated public-literature integrity findings into bounded reviewer-owned tasks.
    pub(super) fn neurosurgery_public_literature_review_queue(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let public_value = arguments.get("public_literature").ok_or_else(|| {
            "neurosurgery_public_literature_review_queue requires public_literature".to_string()
        })?;
        let public_literature: PublicLiteratureBundle =
            serde_json::from_value(public_value.clone())
                .map_err(|error| format!("invalid public-literature bundle: {error}"))?;
        let query = arguments
            .get("query")
            .map(|value| {
                serde_json::from_value::<PublicLiteratureReviewQueueQuery>(value.clone()).map_err(
                    |error| format!("invalid public-literature review-queue query: {error}"),
                )
            })
            .transpose()?
            .unwrap_or_default();
        let report = NeurosurgicalAgent::default()
            .public_literature_review_queue(&public_literature, &query)
            .map_err(|error| format!("public-literature review queue refused: {error}"))?;
        serde_json::to_value(report).map_err(|error| {
            format!("cannot encode public-literature review queue report: {error}")
        })
    }

    /// Join a specialty's explicit research profile to validated real-literature coverage.
    pub(super) fn neurosurgery_public_literature_workbench(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let public_value = arguments.get("public_literature").ok_or_else(|| {
            "neurosurgery_public_literature_workbench requires public_literature".to_string()
        })?;
        let public_literature: PublicLiteratureBundle =
            serde_json::from_value(public_value.clone())
                .map_err(|error| format!("invalid public-literature bundle: {error}"))?;
        let query = arguments
            .get("query")
            .map(|value| {
                serde_json::from_value::<PublicLiteratureWorkbenchQuery>(value.clone())
                    .map_err(|error| format!("invalid public-literature workbench query: {error}"))
            })
            .transpose()?
            .unwrap_or_default();
        let report = NeurosurgicalAgent::default()
            .public_literature_workbench(&public_literature, &query)
            .map_err(|error| format!("public-literature workbench refused: {error}"))?;
        serde_json::to_value(report)
            .map_err(|error| format!("cannot encode public-literature workbench report: {error}"))
    }

    /// Project protocol-defined review tracks onto exact records in one or both validated
    /// snapshots. This is a retrieval agenda, never a clinical interpretation.
    pub(super) fn neurosurgery_evidence_program(&self, arguments: &Value) -> Result<Value, String> {
        let request_value = arguments
            .get("request")
            .ok_or_else(|| "neurosurgery_evidence_program requires request".to_string())?;
        let request: CaseRequest = serde_json::from_value(request_value.clone())
            .map_err(|error| format!("invalid neurosurgical evidence-program request: {error}"))?;
        let query = arguments
            .get("query")
            .map(|value| {
                serde_json::from_value::<EvidenceProgramQuery>(value.clone()).map_err(|error| {
                    format!("invalid neurosurgical evidence-program query: {error}")
                })
            })
            .transpose()?
            .unwrap_or_default();
        let real_data = arguments
            .get("real_glioma_data")
            .map(|value| {
                serde_json::from_value::<RealGliomaBundle>(value.clone())
                    .map_err(|error| format!("invalid neurosurgical real glioma bundle: {error}"))
            })
            .transpose()?;
        let public_literature = arguments
            .get("public_literature")
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
                serde_json::from_value::<CaseAssetManifest>(value.clone())
                    .map_err(|error| format!("invalid neurosurgical case-asset manifest: {error}"))
            })
            .transpose()?;
        let case_asset_query = arguments
            .get("case_asset_manifest_query")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<CaseAssetManifestQuery>(value.clone()).map_err(|error| {
                    format!("invalid neurosurgical case-asset manifest query: {error}")
                })
            })
            .transpose()?;
        let case_asset_disposition = arguments
            .get("case_asset_review_disposition")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<CaseAssetReviewDispositionReport>(value.clone()).map_err(
                    |error| format!("invalid neurosurgical case-asset review disposition: {error}"),
                )
            })
            .transpose()?;
        let agent = NeurosurgicalAgent::default();
        let report = match (
            case_asset_manifest.as_ref(),
            case_asset_disposition.as_ref(),
        ) {
            (Some(manifest), Some(disposition)) => agent
                .evidence_program_with_case_assets_and_dispositions(
                    &request,
                    real_data.as_ref(),
                    public_literature.as_ref(),
                    manifest,
                    &case_asset_query.unwrap_or_default(),
                    disposition,
                    &query,
                ),
            (Some(manifest), None) => agent.evidence_program_with_case_assets(
                &request,
                real_data.as_ref(),
                public_literature.as_ref(),
                manifest,
                &case_asset_query.unwrap_or_default(),
                &query,
            ),
            (None, Some(_)) => Err(bioprism_neurosurgery::NeurosurgeryError::RealDataRejected {
                reason: "case-asset review disposition requires case_asset_manifest".to_string(),
            }),
            (None, None) => agent.evidence_program(
                &request,
                real_data.as_ref(),
                public_literature.as_ref(),
                &query,
            ),
        }
        .map_err(|error| format!("neurosurgical evidence program refused: {error}"))?;
        serde_json::to_value(report)
            .map_err(|error| format!("cannot encode neurosurgical evidence program: {error}"))
    }

    /// Run one bounded, source-linked portfolio pass across selected public-literature lanes.
    pub(super) fn neurosurgery_public_literature_portfolio(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let public_value = arguments.get("public_literature").ok_or_else(|| {
            "neurosurgery_public_literature_portfolio requires public_literature".to_string()
        })?;
        let public_literature: PublicLiteratureBundle =
            serde_json::from_value(public_value.clone())
                .map_err(|error| format!("invalid public-literature bundle: {error}"))?;
        let query = arguments
            .get("query")
            .map(|value| {
                serde_json::from_value::<PublicLiteraturePortfolioQuery>(value.clone())
                    .map_err(|error| format!("invalid public-literature portfolio query: {error}"))
            })
            .transpose()?
            .unwrap_or_default();
        let report = NeurosurgicalAgent::default()
            .public_literature_portfolio(&public_literature, &query)
            .map_err(|error| format!("public-literature portfolio refused: {error}"))?;
        serde_json::to_value(report)
            .map_err(|error| format!("cannot encode public-literature portfolio report: {error}"))
    }

    /// Build a deterministic topic-and-unknowns brief from one validated public bundle. This is
    /// intentionally a local extraction pass, not a model-backed clinical synthesis.
    pub(super) fn neurosurgery_research_brief(&self, arguments: &Value) -> Result<Value, String> {
        let request_value = arguments
            .get("request")
            .ok_or_else(|| "neurosurgery_research_brief requires request".to_string())?;
        let request: CaseRequest = serde_json::from_value(request_value.clone())
            .map_err(|error| format!("invalid neurosurgical research-brief request: {error}"))?;
        let query = arguments
            .get("query")
            .map(|value| {
                serde_json::from_value::<NeurosurgicalResearchBriefQuery>(value.clone())
                    .map_err(|error| format!("invalid neurosurgical research-brief query: {error}"))
            })
            .transpose()?
            .unwrap_or_default();
        let agent = NeurosurgicalAgent::default();
        let report = match (
            arguments.get("real_glioma_data"),
            arguments.get("public_literature"),
        ) {
            (Some(real_data), None) => {
                let real_data: RealGliomaBundle = serde_json::from_value(real_data.clone())
                    .map_err(|error| {
                        format!("invalid neurosurgical real glioma bundle: {error}")
                    })?;
                agent.research_brief(&request, Some(&real_data), None, &query)
            }
            (None, Some(public_literature)) => {
                let public_literature: PublicLiteratureBundle =
                    serde_json::from_value(public_literature.clone()).map_err(|error| {
                        format!("invalid neurosurgical public-literature bundle: {error}")
                    })?;
                agent.research_brief(&request, None, Some(&public_literature), &query)
            }
            (None, None) => Err(bioprism_neurosurgery::NeurosurgeryError::RealDataRejected {
                reason:
                    "neurosurgery_research_brief requires real_glioma_data or public_literature"
                        .to_string(),
            }),
            (Some(_), Some(_)) => Err(bioprism_neurosurgery::NeurosurgeryError::RealDataRejected {
                reason: "neurosurgery_research_brief accepts one evidence bundle, not both"
                    .to_string(),
            }),
        }
        .map_err(|error| format!("neurosurgical research brief refused: {error}"))?;
        serde_json::to_value(report)
            .map_err(|error| format!("cannot encode neurosurgical research brief: {error}"))
    }

    /// Compile a bounded, source-linked research handoff from an intake request and an optional
    /// validated snapshot. This never fetches sources or treats population metadata as case data.
    pub(super) fn neurosurgery_research_plan(&self, arguments: &Value) -> Result<Value, String> {
        let request_value = arguments
            .get("request")
            .ok_or_else(|| "neurosurgery_research_plan requires request".to_string())?;
        let request: CaseRequest = serde_json::from_value(request_value.clone())
            .map_err(|error| format!("invalid neurosurgical research-plan request: {error}"))?;
        if arguments.get("real_glioma_data").is_some()
            && arguments.get("public_literature").is_some()
        {
            return Err(
                "neurosurgery_research_plan accepts one evidence bundle: choose real_glioma_data or public_literature"
                    .to_string(),
            );
        }
        let max_tasks = arguments
            .get("max_tasks")
            .map(|value| {
                value
                    .as_u64()
                    .and_then(|value| usize::try_from(value).ok())
                    .ok_or_else(|| "max_tasks must be an integer".to_string())
            })
            .transpose()?
            .unwrap_or(8);
        let max_references = arguments
            .get("max_references_per_task")
            .map(|value| {
                value
                    .as_u64()
                    .and_then(|value| usize::try_from(value).ok())
                    .ok_or_else(|| "max_references_per_task must be an integer".to_string())
            })
            .transpose()?
            .unwrap_or(4);
        if max_tasks == 0 || max_tasks > MAX_RESEARCH_PLAN_TASKS {
            return Err(format!(
                "max_tasks must be between 1 and {MAX_RESEARCH_PLAN_TASKS}"
            ));
        }
        if max_references == 0 || max_references > MAX_RESEARCH_PLAN_REFERENCES {
            return Err(format!(
                "max_references_per_task must be between 1 and {MAX_RESEARCH_PLAN_REFERENCES}"
            ));
        }
        let real_data = arguments
            .get("real_glioma_data")
            .map(|value| {
                serde_json::from_value::<RealGliomaBundle>(value.clone())
                    .map_err(|error| format!("invalid real glioma planning data: {error}"))
            })
            .transpose()?;
        let public_literature = arguments
            .get("public_literature")
            .map(|value| {
                serde_json::from_value::<PublicLiteratureBundle>(value.clone())
                    .map_err(|error| format!("invalid public literature planning data: {error}"))
            })
            .transpose()?;
        let report = NeurosurgicalAgent::default()
            .plan_research(
                &request,
                real_data.as_ref(),
                public_literature.as_ref(),
                max_tasks,
                max_references,
            )
            .map_err(|error| format!("neurosurgical research plan refused: {error}"))?;
        serde_json::to_value(report)
            .map_err(|error| format!("cannot encode neurosurgical research plan: {error}"))
    }

    /// Compile a bounded, dual-plane local acquisition wave from explicit evidence gaps.
    pub(super) fn neurosurgery_evidence_acquisition(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let operation = match arguments.get("operation") {
            Some(value) => value
                .as_str()
                .ok_or_else(|| "evidence-acquisition operation must be a string".to_string())?,
            None => "compile",
        };
        if !matches!(operation, "compile" | "start" | "advance" | "finish") {
            return Err(format!(
                "unsupported evidence-acquisition operation `{operation}`; expected compile, start, advance, or finish"
            ));
        }
        let request_value = arguments
            .get("request")
            .ok_or_else(|| "neurosurgery_evidence_acquisition requires request".to_string())?;
        let request: CaseRequest = serde_json::from_value(request_value.clone())
            .map_err(|error| format!("invalid neurosurgical acquisition request: {error}"))?;
        let query = arguments
            .get("query")
            .map(|value| {
                serde_json::from_value::<EvidenceAcquisitionQuery>(value.clone())
                    .map_err(|error| format!("invalid evidence-acquisition query: {error}"))
            })
            .transpose()?
            .unwrap_or_default();
        if query.max_steps == 0 || query.max_steps > MAX_EVIDENCE_ACQUISITION_STEPS {
            return Err(format!(
                "max_steps must be between 1 and {MAX_EVIDENCE_ACQUISITION_STEPS}"
            ));
        }
        if query.max_references_per_step == 0
            || query.max_references_per_step > MAX_EVIDENCE_ACQUISITION_REFERENCES
        {
            return Err(format!(
                "max_references_per_step must be between 1 and {MAX_EVIDENCE_ACQUISITION_REFERENCES}"
            ));
        }
        let real_data = arguments
            .get("real_glioma_data")
            .map(|value| {
                serde_json::from_value::<RealGliomaBundle>(value.clone())
                    .map_err(|error| format!("invalid real glioma acquisition data: {error}"))
            })
            .transpose()?;
        let public_literature = arguments
            .get("public_literature")
            .map(|value| {
                serde_json::from_value::<PublicLiteratureBundle>(value.clone())
                    .map_err(|error| format!("invalid public literature acquisition data: {error}"))
            })
            .transpose()?;
        let case_asset_manifest = arguments
            .get("case_asset_manifest")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<CaseAssetManifest>(value.clone())
                    .map_err(|error| format!("invalid case asset acquisition manifest: {error}"))
            })
            .transpose()?;
        let case_asset_query = arguments
            .get("case_asset_manifest_query")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<CaseAssetManifestQuery>(value.clone()).map_err(|error| {
                    format!("invalid case asset acquisition manifest query: {error}")
                })
            })
            .transpose()?;
        let case_asset_disposition = arguments
            .get("case_asset_review_disposition")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<CaseAssetReviewDispositionReport>(value.clone())
                    .map_err(|error| format!("invalid case asset acquisition disposition: {error}"))
            })
            .transpose()?;
        if case_asset_query.is_some() && case_asset_manifest.is_none() {
            return Err(
                "neurosurgery_evidence_acquisition case_asset_manifest_query requires case_asset_manifest"
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
                        format!("neurosurgical case asset projection refused: {error}")
                    })
            })
            .transpose()?;
        let value = match operation {
            "compile" => {
                let report = match case_asset_disposition.as_ref() {
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
                }
                .map_err(|error| format!("neurosurgical evidence acquisition refused: {error}"))?;
                serde_json::to_value(report)
            }
            "start" => {
                let result = match case_asset_disposition.as_ref() {
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
                }
                .map_err(|error| {
                    format!("neurosurgical evidence acquisition start refused: {error}")
                })?;
                serde_json::to_value(result)
            }
            "advance" => {
                let session_value = arguments.get("session").ok_or_else(|| {
                    "neurosurgery_evidence_acquisition advance requires session".to_string()
                })?;
                let session: EvidenceAcquisitionSession =
                    serde_json::from_value(session_value.clone()).map_err(|error| {
                        format!("invalid evidence-acquisition session: {error}")
                    })?;
                let max_steps = match arguments.get("max_steps") {
                    None => 1,
                    Some(value) => usize::try_from(value.as_u64().ok_or_else(|| {
                        "evidence-acquisition advance max_steps must be an integer".to_string()
                    })?)
                    .map_err(|_| {
                        "evidence-acquisition advance max_steps exceeds platform bounds".to_string()
                    })?,
                };
                if !(1..=MAX_EVIDENCE_ACQUISITION_ADVANCE_STEPS).contains(&max_steps) {
                    return Err(format!(
                        "advance max_steps must be between 1 and {MAX_EVIDENCE_ACQUISITION_ADVANCE_STEPS}"
                    ));
                }
                let result = match case_asset_disposition.as_ref() {
                    Some(disposition) => agent
                        .evidence_acquisition_advance_with_case_assets_and_dispositions(
                            &session,
                            &request,
                            real_data.as_ref(),
                            public_literature.as_ref(),
                            case_asset_report.as_ref(),
                            disposition,
                            &query,
                            max_steps,
                        ),
                    None => agent.evidence_acquisition_advance_with_case_assets(
                        &session,
                        &request,
                        real_data.as_ref(),
                        public_literature.as_ref(),
                        case_asset_report.as_ref(),
                        &query,
                        max_steps,
                    ),
                }
                .map_err(|error| {
                    format!("neurosurgical evidence acquisition advance refused: {error}")
                })?;
                serde_json::to_value(result)
            }
            "finish" => {
                let session_value = arguments.get("session").ok_or_else(|| {
                    "neurosurgery_evidence_acquisition finish requires session".to_string()
                })?;
                let session: EvidenceAcquisitionSession =
                    serde_json::from_value(session_value.clone()).map_err(|error| {
                        format!("invalid evidence-acquisition session: {error}")
                    })?;
                let result = match case_asset_disposition.as_ref() {
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
                }
                .map_err(|error| {
                    format!("neurosurgical evidence acquisition finish refused: {error}")
                })?;
                serde_json::to_value(result)
            }
            _ => unreachable!(),
        };
        value.map_err(|error| format!("cannot encode neurosurgical evidence acquisition: {error}"))
    }
}
