//! Neurosurgery evidence, literature, and mission handlers.

use super::*;

impl Server {
    /// Query a caller-supplied, validated public glioma bundle without network access.
    pub(super) fn neurosurgery_real_data_query(&self, arguments: &Value) -> Result<Value, String> {
        let data_value = arguments
            .get("real_glioma_data")
            .ok_or_else(|| "neurosurgery_real_data_query requires real_glioma_data".to_string())?;
        let data: RealGliomaBundle = serde_json::from_value(data_value.clone())
            .map_err(|error| format!("invalid real glioma data bundle: {error}"))?;
        let query_value = arguments
            .get("query")
            .ok_or_else(|| "neurosurgery_real_data_query requires query".to_string())?;
        let query: RealDataQuery = serde_json::from_value(query_value.clone())
            .map_err(|error| format!("invalid real-data query: {error}"))?;
        let result = data
            .query(&query)
            .map_err(|error| format!("real-data query refused: {error}"))?;
        serde_json::to_value(result)
            .map_err(|error| format!("cannot encode real-data query result: {error}"))
    }

    /// Build a bounded, descriptive ClinicalTrials.gov landscape from a local public snapshot.
    pub(super) fn neurosurgery_real_data_trial_landscape(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let data_value = arguments.get("real_glioma_data").ok_or_else(|| {
            "neurosurgery_real_data_trial_landscape requires real_glioma_data".to_string()
        })?;
        let data: RealGliomaBundle = serde_json::from_value(data_value.clone())
            .map_err(|error| format!("invalid real glioma data bundle: {error}"))?;
        let query_value = arguments.get("query").cloned().unwrap_or_else(|| json!({}));
        let query: RealDataTrialLandscapeQuery = serde_json::from_value(query_value)
            .map_err(|error| format!("invalid trial-landscape query: {error}"))?;
        let result = data
            .trial_landscape(&query)
            .map_err(|error| format!("trial landscape refused: {error}"))?;
        serde_json::to_value(result)
            .map_err(|error| format!("cannot encode trial landscape: {error}"))
    }

    /// Build a bounded, descriptive cBioPortal molecular-assay plus aggregate GDC availability
    /// inventory from a local public snapshot. The result contains metadata only and never values.
    pub(super) fn neurosurgery_real_data_molecular_coverage(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let data_value = arguments.get("real_glioma_data").ok_or_else(|| {
            "neurosurgery_real_data_molecular_coverage requires real_glioma_data".to_string()
        })?;
        let data: RealGliomaBundle = serde_json::from_value(data_value.clone())
            .map_err(|error| format!("invalid real glioma data bundle: {error}"))?;
        let query_value = arguments.get("query").cloned().unwrap_or_else(|| json!({}));
        let query: RealDataMolecularCoverageQuery = serde_json::from_value(query_value)
            .map_err(|error| format!("invalid molecular-coverage query: {error}"))?;
        let result = data
            .molecular_coverage(&query)
            .map_err(|error| format!("molecular coverage refused: {error}"))?;
        serde_json::to_value(result)
            .map_err(|error| format!("cannot encode molecular coverage: {error}"))
    }

    /// Query a caller-supplied cross-specialty PubMed snapshot without network access.
    pub(super) fn neurosurgery_public_literature_query(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let data_value = arguments.get("public_literature").ok_or_else(|| {
            "neurosurgery_public_literature_query requires public_literature".to_string()
        })?;
        let data: PublicLiteratureBundle = serde_json::from_value(data_value.clone())
            .map_err(|error| format!("invalid public-literature bundle: {error}"))?;
        let query_value = arguments
            .get("query")
            .ok_or_else(|| "neurosurgery_public_literature_query requires query".to_string())?;
        let query: PublicLiteratureQuery = serde_json::from_value(query_value.clone())
            .map_err(|error| format!("invalid public-literature query: {error}"))?;
        let result = data
            .query(&query)
            .map_err(|error| format!("public-literature query refused: {error}"))?;
        serde_json::to_value(result)
            .map_err(|error| format!("cannot encode public-literature query result: {error}"))
    }

    /// Advance the stateless neurosurgical session lifecycle. The caller owns the checkpoint and
    /// must send the same request/data bytes at every step; the Rust core verifies their digests.
    pub(super) fn neurosurgery_session(&self, arguments: &Value) -> Result<Value, String> {
        let operation = arguments
            .get("operation")
            .and_then(Value::as_str)
            .ok_or_else(|| "neurosurgery_session requires operation".to_string())?;
        let request_value = arguments
            .get("request")
            .ok_or_else(|| "neurosurgery_session requires request".to_string())?;
        let request: CaseRequest = serde_json::from_value(request_value.clone())
            .map_err(|error| format!("invalid neurosurgical session request: {error}"))?;
        if operation != "run" && arguments.get("max_steps").is_some() {
            return Err(
                "max_steps is only accepted for neurosurgery_session operation=run".to_string(),
            );
        }
        let real_data = arguments
            .get("real_glioma_data")
            .map(|value| {
                serde_json::from_value::<RealGliomaBundle>(value.clone())
                    .map_err(|error| format!("invalid real glioma session data: {error}"))
            })
            .transpose()?;
        let public_literature = arguments
            .get("public_literature")
            .map(|value| {
                serde_json::from_value::<PublicLiteratureBundle>(value.clone())
                    .map_err(|error| format!("invalid public literature session data: {error}"))
            })
            .transpose()?;
        if real_data.is_some() && public_literature.is_some() {
            return Err("neurosurgery_session accepts one evidence bundle: choose real_glioma_data or public_literature".to_string());
        }
        let agent = NeurosurgicalAgent::default();
        let value = match operation {
            "start" => if let Some(literature) = public_literature.as_ref() {
                agent.start_session_with_public_literature(&request, literature)
            } else {
                agent.start_session(&request, real_data.as_ref())
            }
            .map_err(|error| format!("neurosurgical session start refused: {error}"))
            .and_then(|session| {
                serde_json::to_value(session)
                    .map_err(|error| format!("cannot encode session start: {error}"))
            })?,
            "advance" => {
                let session_value = arguments
                    .get("session")
                    .ok_or_else(|| "advance requires session".to_string())?;
                let session: NeurosurgicalSession = serde_json::from_value(session_value.clone())
                    .map_err(|error| {
                    format!("invalid neurosurgical session checkpoint: {error}")
                })?;
                let result = if let Some(literature) = public_literature.as_ref() {
                    agent.advance_session_with_public_literature(&session, &request, literature)
                } else {
                    agent.advance_session(&session, &request, real_data.as_ref())
                };
                result
                    .map_err(|error| format!("neurosurgical session advance refused: {error}"))
                    .and_then(|session| {
                        serde_json::to_value(session)
                            .map_err(|error| format!("cannot encode session advance: {error}"))
                    })?
            }
            "run" => {
                let max_steps = match arguments.get("max_steps") {
                    None => MAX_SESSION_STEPS,
                    Some(value) => value
                        .as_u64()
                        .and_then(|value| usize::try_from(value).ok())
                        .ok_or_else(|| "max_steps must be an integer".to_string())?,
                };
                let result = if let Some(literature) = public_literature.as_ref() {
                    agent.run_session_to_review_with_public_literature(
                        &request, literature, max_steps,
                    )
                } else {
                    agent.run_session_to_review(&request, real_data.as_ref(), max_steps)
                };
                result
                    .map_err(|error| format!("neurosurgical session run refused: {error}"))
                    .and_then(|result| {
                        serde_json::to_value(result)
                            .map_err(|error| format!("cannot encode session run: {error}"))
                    })?
            }
            "finish" => {
                let session_value = arguments
                    .get("session")
                    .ok_or_else(|| "finish requires session".to_string())?;
                let session: NeurosurgicalSession = serde_json::from_value(session_value.clone())
                    .map_err(|error| {
                    format!("invalid neurosurgical session checkpoint: {error}")
                })?;
                let result = if let Some(literature) = public_literature.as_ref() {
                    agent.finish_session_with_public_literature(&session, &request, literature)
                } else {
                    agent.finish_session(&session, &request, real_data.as_ref())
                };
                result
                    .map_err(|error| format!("neurosurgical session finish refused: {error}"))
                    .and_then(|response| {
                        serde_json::to_value(response)
                            .map_err(|error| format!("cannot encode session finish: {error}"))
                    })?
            }
            other => {
                return Err(format!(
                    "unknown neurosurgery_session operation {other:?}; expected start, advance, run, or finish"
                ));
            }
        };
        serde_json::to_value(value)
            .map_err(|error| format!("cannot encode neurosurgical session response: {error}"))
    }

    /// Compose catalogue discovery, an optional public-bundle query, and the bounded session
    /// worker into one provider-free mission envelope. The caller still owns the bundle and no
    /// network or model is contacted by this convenience operation.
    pub(super) fn neurosurgery_mission(&self, arguments: &Value) -> Result<Value, String> {
        let request_value = arguments
            .get("request")
            .ok_or_else(|| "neurosurgery_mission requires request".to_string())?;
        let request: CaseRequest = serde_json::from_value(request_value.clone())
            .map_err(|error| format!("invalid neurosurgical mission request: {error}"))?;
        let operation = arguments
            .get("operation")
            .and_then(Value::as_str)
            .unwrap_or("run");
        if operation == "validate" {
            let mission_value = arguments.get("mission").ok_or_else(|| {
                "neurosurgery_mission operation=validate requires mission".to_string()
            })?;
            let mission: NeurosurgicalMissionResult = serde_json::from_value(mission_value.clone())
                .map_err(|error| format!("invalid persisted neurosurgical mission: {error}"))?;
            let real_data = arguments
                .get("real_glioma_data")
                .map(|value| {
                    serde_json::from_value::<RealGliomaBundle>(value.clone())
                        .map_err(|error| format!("invalid neurosurgical mission data: {error}"))
                })
                .transpose()?;
            let public_literature = arguments
                .get("public_literature")
                .map(|value| {
                    serde_json::from_value::<PublicLiteratureBundle>(value.clone()).map_err(
                        |error| format!("invalid neurosurgical mission literature: {error}"),
                    )
                })
                .transpose()?;
            let case_dicom_import = arguments
                .get("case_dicom_import")
                .filter(|value| !value.is_null())
                .map(|value| {
                    serde_json::from_value::<DicomCaseImport>(value.clone()).map_err(|error| {
                        format!("invalid neurosurgical mission DICOM replay import: {error}")
                    })
                })
                .transpose()?;
            let case_fhir_import = arguments
                .get("case_fhir_import")
                .filter(|value| !value.is_null())
                .map(|value| {
                    serde_json::from_value::<FhirCaseImport>(value.clone()).map_err(|error| {
                        format!("invalid neurosurgical mission FHIR replay import: {error}")
                    })
                })
                .transpose()?;
            mission
                .validate_for_inputs_with_case_imports(
                    &request,
                    real_data.as_ref(),
                    public_literature.as_ref(),
                    case_dicom_import.as_ref(),
                    case_fhir_import.as_ref(),
                )
                .map_err(|error| format!("neurosurgical mission replay refused: {error}"))?;
            return serde_json::to_value(serde_json::json!({
                "valid": true,
                "mission_id": mission.mission_id,
                "specialty": mission.specialty,
                "status": mission.status,
                "human_review_required": mission.human_review_required,
                "request_digest": mission.run.response.request_digest,
                "audit_digest": mission.mission_audit.as_ref().map(|audit| audit.audit_digest.clone()),
                "provider": mission.provider,
                "network": mission.network,
            }))
            .map_err(|error| format!("cannot encode neurosurgical mission validation: {error}"));
        }
        if operation != "run" {
            return Err(format!(
                "unknown neurosurgery_mission operation {operation:?}; expected run or validate"
            ));
        }
        let real_data = arguments
            .get("real_glioma_data")
            .map(|value| {
                serde_json::from_value::<RealGliomaBundle>(value.clone())
                    .map_err(|error| format!("invalid neurosurgical mission data: {error}"))
            })
            .transpose()?;
        let public_literature = arguments
            .get("public_literature")
            .map(|value| {
                serde_json::from_value::<PublicLiteratureBundle>(value.clone())
                    .map_err(|error| format!("invalid neurosurgical mission literature: {error}"))
            })
            .transpose()?;
        let case_asset_manifest = arguments
            .get("case_asset_manifest")
            .map(|value| {
                serde_json::from_value::<CaseAssetManifest>(value.clone()).map_err(|error| {
                    format!("invalid neurosurgical mission case asset manifest: {error}")
                })
            })
            .transpose()?;
        let case_dicom_import = arguments
            .get("case_dicom_import")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<DicomCaseImport>(value.clone())
                    .map_err(|error| format!("invalid neurosurgical mission DICOM import: {error}"))
            })
            .transpose()?;
        let case_fhir_import = arguments
            .get("case_fhir_import")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<FhirCaseImport>(value.clone())
                    .map_err(|error| format!("invalid neurosurgical mission FHIR import: {error}"))
            })
            .transpose()?;
        let case_asset_query = arguments
            .get("case_asset_manifest_query")
            .map(|value| {
                serde_json::from_value::<CaseAssetManifestQuery>(value.clone()).map_err(|error| {
                    format!("invalid neurosurgical mission case asset query: {error}")
                })
            })
            .transpose()?;
        let case_asset_disposition = arguments
            .get("case_asset_review_disposition")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<CaseAssetReviewDispositionReport>(value.clone()).map_err(
                    |error| {
                        format!("invalid neurosurgical mission case asset disposition: {error}")
                    },
                )
            })
            .transpose()?;
        // A mission may bind both validated bundles. In that mode the real glioma snapshot
        // drives the session route and the PubMed snapshot contributes independent citation
        // context plus an exact PMID/DOI linkage audit.
        let query_value = arguments.get("query");
        let query = if real_data.is_some() {
            query_value
                .map(|value| {
                    serde_json::from_value::<RealDataQuery>(value.clone())
                        .map_err(|error| format!("invalid neurosurgical mission query: {error}"))
                })
                .transpose()?
        } else {
            None
        };
        let public_query = if public_literature.is_some() {
            arguments
                .get("public_literature_query")
                .or_else(|| {
                    if real_data.is_none() {
                        query_value
                    } else {
                        None
                    }
                })
                .map(|value| {
                    serde_json::from_value::<PublicLiteratureQuery>(value.clone()).map_err(
                        |error| format!("invalid public-literature mission query: {error}"),
                    )
                })
                .transpose()?
        } else {
            None
        };
        let portfolio_query = arguments
            .get("portfolio_query")
            .map(|value| {
                serde_json::from_value::<PublicLiteraturePortfolioQuery>(value.clone()).map_err(
                    |error| format!("invalid public-literature mission portfolio query: {error}"),
                )
            })
            .transpose()?;
        let freshness_value = arguments.get("freshness");
        let freshness = freshness_value
            .map(|value| {
                serde_json::from_value::<RealDataFreshnessQuery>(value.clone()).map_err(|error| {
                    format!("invalid neurosurgical mission freshness query: {error}")
                })
            })
            .transpose()?;
        if freshness.is_some() && real_data.is_none() && public_literature.is_none() {
            return Err(
                "neurosurgery_mission freshness requires real_glioma_data or public_literature"
                    .to_string(),
            );
        }
        if query_value.is_some() && real_data.is_none() && public_literature.is_none() {
            return Err(
                "neurosurgery_mission query requires real_glioma_data or public_literature"
                    .to_string(),
            );
        }
        if portfolio_query.is_some() && public_literature.is_none() {
            return Err(
                "neurosurgery_mission portfolio_query requires public_literature".to_string(),
            );
        }
        let max_steps = arguments
            .get("max_steps")
            .map(|value| {
                value
                    .as_u64()
                    .and_then(|value| usize::try_from(value).ok())
                    .ok_or_else(|| "neurosurgery_mission max_steps must be an integer".to_string())
            })
            .transpose()?
            .unwrap_or(MAX_SESSION_STEPS);
        if case_dicom_import.is_some() {
            if case_asset_manifest.is_some() {
                return Err(
                    "neurosurgery_mission accepts either case_dicom_import or case_asset_manifest, not both"
                        .to_string(),
                );
            }
            if case_asset_query.is_some() || case_asset_disposition.is_some() {
                return Err(
                    "neurosurgery_mission DICOM imports do not accept a separate asset query or disposition"
                        .to_string(),
                );
            }
            if real_data.is_none() {
                return Err(
                    "neurosurgery_mission case_dicom_import requires real_glioma_data".to_string(),
                );
            }
            if public_literature.is_some() && case_fhir_import.is_none() {
                return Err(
                    "neurosurgery_mission case_dicom_import with public_literature also requires case_fhir_import"
                        .to_string(),
                );
            }
        }
        if case_fhir_import.is_some() {
            if case_asset_manifest.is_some() {
                return Err(
                    "neurosurgery_mission accepts case_fhir_import or case_dicom_import, not a separate case_asset_manifest"
                        .to_string(),
                );
            }
            if case_asset_query.is_some() || case_asset_disposition.is_some() {
                return Err(
                    "neurosurgery_mission FHIR imports do not accept a separate asset query or disposition"
                        .to_string(),
                );
            }
        }
        let agent = NeurosurgicalAgent::default();
        let mission = if case_dicom_import.is_some() && case_fhir_import.is_some() {
            agent.run_research_mission_with_case_imports(
                &request,
                real_data.as_ref(),
                public_literature.as_ref(),
                query.as_ref(),
                public_query.as_ref(),
                freshness.as_ref(),
                portfolio_query.as_ref(),
                case_dicom_import.as_ref(),
                case_fhir_import.as_ref(),
                max_steps,
            )
        } else if let Some(dicom_import) = case_dicom_import.as_ref() {
            let real_data = real_data
                .as_ref()
                .expect("DICOM mission validation requires real data");
            agent.run_research_mission_with_case_dicom(
                &request,
                real_data,
                query.as_ref(),
                freshness.as_ref(),
                dicom_import,
                max_steps,
            )
        } else if let Some(fhir_import) = case_fhir_import.as_ref() {
            agent.run_research_mission_with_case_fhir(
                &request,
                real_data.as_ref(),
                public_literature.as_ref(),
                query.as_ref(),
                public_query.as_ref(),
                freshness.as_ref(),
                portfolio_query.as_ref(),
                fhir_import,
                max_steps,
            )
        } else if let (Some(real_data), Some(literature)) =
            (real_data.as_ref(), public_literature.as_ref())
        {
            let real_query = query_value
                .map(|value| {
                    serde_json::from_value::<RealDataQuery>(value.clone())
                        .map_err(|error| format!("invalid real-data mission query: {error}"))
                })
                .transpose()?;
            agent.run_research_mission_with_real_data_and_public_literature_case_assets_and_dispositions(
                // The real-data route remains canonical; the optional asset plane is projected
                // into the same mission envelope without changing session route semantics.
                &request,
                real_data,
                literature,
                real_query.as_ref(),
                public_query.as_ref(),
                freshness.as_ref(),
                portfolio_query.as_ref(),
                case_asset_manifest.as_ref(),
                case_asset_query.as_ref(),
                case_asset_disposition.as_ref(),
                max_steps,
            )
        } else if let Some(literature) = public_literature.as_ref() {
            agent.run_research_mission_with_public_literature_case_assets_and_dispositions(
                &request,
                literature,
                public_query.as_ref(),
                freshness.as_ref(),
                portfolio_query.as_ref(),
                case_asset_manifest.as_ref(),
                case_asset_query.as_ref(),
                case_asset_disposition.as_ref(),
                max_steps,
            )
        } else {
            agent.run_research_mission_with_case_assets_and_dispositions(
                &request,
                real_data.as_ref(),
                query.as_ref(),
                freshness.as_ref(),
                case_asset_manifest.as_ref(),
                case_asset_query.as_ref(),
                case_asset_disposition.as_ref(),
                max_steps,
            )
        }
        .map_err(|error| format!("neurosurgical mission run refused: {error}"))?;
        serde_json::to_value(mission)
            .map_err(|error| format!("cannot encode neurosurgical mission response: {error}"))
    }
}
