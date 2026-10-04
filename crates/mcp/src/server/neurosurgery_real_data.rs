//! Neurosurgery evidence, literature, and mission handlers.

use super::*;

impl Server {
    /// Project source, temporal, assay, and linkage coverage from a validated real glioma bundle.
    pub(super) fn neurosurgery_real_data_coverage(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let data_value = arguments.get("real_glioma_data").ok_or_else(|| {
            "neurosurgery_real_data_coverage requires real_glioma_data".to_string()
        })?;
        let real_glioma_data: RealGliomaBundle = serde_json::from_value(data_value.clone())
            .map_err(|error| format!("invalid neurosurgical real glioma bundle: {error}"))?;
        let query = arguments
            .get("query")
            .map(|value| {
                serde_json::from_value::<RealDataCoverageQuery>(value.clone()).map_err(|error| {
                    format!("invalid neurosurgical real-data coverage query: {error}")
                })
            })
            .transpose()?
            .unwrap_or_default();
        let report = NeurosurgicalAgent::default()
            .real_data_coverage(&real_glioma_data, &query)
            .map_err(|error| format!("neurosurgical real-data coverage refused: {error}"))?;
        serde_json::to_value(report).map_err(|error| {
            format!("cannot encode neurosurgical real-data coverage report: {error}")
        })
    }

    /// Compare aggregate genomic projects and file-type availability without opening source
    /// files or treating released-case inventory as patient-level evidence.
    pub(super) fn neurosurgery_real_data_cohort_landscape(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let data_value = arguments.get("real_glioma_data").ok_or_else(|| {
            "neurosurgery_real_data_cohort_landscape requires real_glioma_data".to_string()
        })?;
        let real_glioma_data: RealGliomaBundle = serde_json::from_value(data_value.clone())
            .map_err(|error| format!("invalid neurosurgical real glioma bundle: {error}"))?;
        let query = arguments
            .get("query")
            .map(|value| {
                serde_json::from_value::<RealDataCohortLandscapeQuery>(value.clone()).map_err(
                    |error| format!("invalid neurosurgical cohort-landscape query: {error}"),
                )
            })
            .transpose()?
            .unwrap_or_default();
        let report = NeurosurgicalAgent::default()
            .real_data_cohort_landscape(&real_glioma_data, &query)
            .map_err(|error| format!("neurosurgical cohort landscape refused: {error}"))?;
        serde_json::to_value(report).map_err(|error| {
            format!("cannot encode neurosurgical cohort landscape report: {error}")
        })
    }

    /// Reconcile exact PMID/DOI identifiers inside a validated real glioma snapshot.
    pub(super) fn neurosurgery_real_data_reconciliation(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let data_value = arguments.get("real_glioma_data").ok_or_else(|| {
            "neurosurgery_real_data_reconciliation requires real_glioma_data".to_string()
        })?;
        let real_glioma_data: RealGliomaBundle = serde_json::from_value(data_value.clone())
            .map_err(|error| format!("invalid neurosurgical real glioma bundle: {error}"))?;
        let query = arguments
            .get("query")
            .map(|value| {
                serde_json::from_value::<RealDataReconciliationQuery>(value.clone()).map_err(
                    |error| {
                        format!("invalid neurosurgical real-data reconciliation query: {error}")
                    },
                )
            })
            .transpose()?
            .unwrap_or_default();
        let report = NeurosurgicalAgent::default()
            .real_data_reconciliation(&real_glioma_data, &query)
            .map_err(|error| format!("neurosurgical real-data reconciliation refused: {error}"))?;
        serde_json::to_value(report).map_err(|error| {
            format!("cannot encode neurosurgical real-data reconciliation report: {error}")
        })
    }

    /// Report source-age posture for a validated real-glioma bundle using an explicit caller-owned
    /// as-of timestamp. Age is not treated as quality, applicability, or clinical relevance.
    pub(super) fn neurosurgery_real_data_freshness(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let data_value = arguments.get("real_glioma_data").ok_or_else(|| {
            "neurosurgery_real_data_freshness requires real_glioma_data".to_string()
        })?;
        let real_glioma_data: RealGliomaBundle = serde_json::from_value(data_value.clone())
            .map_err(|error| format!("invalid neurosurgical real glioma bundle: {error}"))?;
        let query_value = arguments
            .get("query")
            .ok_or_else(|| "neurosurgery_real_data_freshness requires query".to_string())?;
        let query: RealDataFreshnessQuery = serde_json::from_value(query_value.clone())
            .map_err(|error| format!("invalid neurosurgical real-data freshness query: {error}"))?;
        let report = NeurosurgicalAgent::default()
            .real_data_freshness(&real_glioma_data, &query)
            .map_err(|error| format!("neurosurgical real-data freshness refused: {error}"))?;
        serde_json::to_value(report).map_err(|error| {
            format!("cannot encode neurosurgical real-data freshness report: {error}")
        })
    }

    /// Compare two validated public glioma snapshots without fetching or interpreting records.
    pub(super) fn neurosurgery_real_data_diff(&self, arguments: &Value) -> Result<Value, String> {
        let before_value = arguments.get("before_real_glioma_data").ok_or_else(|| {
            "neurosurgery_real_data_diff requires before_real_glioma_data".to_string()
        })?;
        let after_value = arguments.get("after_real_glioma_data").ok_or_else(|| {
            "neurosurgery_real_data_diff requires after_real_glioma_data".to_string()
        })?;
        let before: RealGliomaBundle = serde_json::from_value(before_value.clone())
            .map_err(|error| format!("invalid before real glioma bundle: {error}"))?;
        let after: RealGliomaBundle = serde_json::from_value(after_value.clone())
            .map_err(|error| format!("invalid after real glioma bundle: {error}"))?;
        let query = arguments
            .get("query")
            .map(|value| {
                serde_json::from_value::<RealDataDiffQuery>(value.clone())
                    .map_err(|error| format!("invalid neurosurgical real-data diff query: {error}"))
            })
            .transpose()?
            .unwrap_or_default();
        if query.max_changes == 0 || query.max_changes > MAX_REAL_DATA_DIFF_CHANGES {
            return Err(format!(
                "query.max_changes must be between 1 and {MAX_REAL_DATA_DIFF_CHANGES}"
            ));
        }
        let report = NeurosurgicalAgent::default()
            .real_data_diff(&before, &after, &query)
            .map_err(|error| format!("neurosurgical real-data diff refused: {error}"))?;
        serde_json::to_value(report)
            .map_err(|error| format!("cannot encode neurosurgical real-data diff report: {error}"))
    }

    /// Compose the deterministic refresh/reconciliation reports for two validated real-glioma
    /// snapshots. The candidate is never merged or accepted by the MCP boundary.
    pub(super) fn neurosurgery_real_data_refresh_audit(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request_value = arguments
            .get("request")
            .ok_or_else(|| "neurosurgery_real_data_refresh_audit requires request".to_string())?;
        let request: CaseRequest = serde_json::from_value(request_value.clone())
            .map_err(|error| format!("invalid neurosurgical refresh-audit request: {error}"))?;
        let before_value = arguments.get("before_real_glioma_data").ok_or_else(|| {
            "neurosurgery_real_data_refresh_audit requires before_real_glioma_data".to_string()
        })?;
        let after_value = arguments.get("after_real_glioma_data").ok_or_else(|| {
            "neurosurgery_real_data_refresh_audit requires after_real_glioma_data".to_string()
        })?;
        let before: RealGliomaBundle = serde_json::from_value(before_value.clone())
            .map_err(|error| format!("invalid before real glioma bundle: {error}"))?;
        let after: RealGliomaBundle = serde_json::from_value(after_value.clone())
            .map_err(|error| format!("invalid after real glioma bundle: {error}"))?;
        let query = arguments
            .get("query")
            .map(|value| {
                serde_json::from_value::<RealDataRefreshAuditQuery>(value.clone())
                    .map_err(|error| format!("invalid neurosurgical refresh-audit query: {error}"))
            })
            .transpose()?
            .unwrap_or_default();
        let report = NeurosurgicalAgent::default()
            .real_data_refresh_audit(&request, &before, &after, &query)
            .map_err(|error| format!("neurosurgical real-data refresh audit refused: {error}"))?;
        serde_json::to_value(report).map_err(|error| {
            format!("cannot encode neurosurgical real-data refresh audit report: {error}")
        })
    }

    /// Derive explicit metadata-review obligations from one validated public glioma snapshot.
    pub(super) fn neurosurgery_real_data_review_queue(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let data_value = arguments.get("real_glioma_data").ok_or_else(|| {
            "neurosurgery_real_data_review_queue requires real_glioma_data".to_string()
        })?;
        let real_glioma_data: RealGliomaBundle = serde_json::from_value(data_value.clone())
            .map_err(|error| format!("invalid neurosurgical real glioma bundle: {error}"))?;
        let query = arguments
            .get("query")
            .map(|value| {
                serde_json::from_value::<RealDataReviewQueueQuery>(value.clone()).map_err(|error| {
                    format!("invalid neurosurgical real-data review-queue query: {error}")
                })
            })
            .transpose()?
            .unwrap_or_default();
        if query.max_items == 0 || query.max_items > MAX_REAL_DATA_REVIEW_ITEMS {
            return Err(format!(
                "query.max_items must be between 1 and {MAX_REAL_DATA_REVIEW_ITEMS}"
            ));
        }
        let report = NeurosurgicalAgent::default()
            .real_data_review_queue(&real_glioma_data, &query)
            .map_err(|error| format!("neurosurgical real-data review queue refused: {error}"))?;
        serde_json::to_value(report).map_err(|error| {
            format!("cannot encode neurosurgical real-data review queue report: {error}")
        })
    }

    /// Apply caller-owned, digest-bound dispositions to emitted review tasks.
    pub(super) fn neurosurgery_real_data_review_disposition(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let queue_value = arguments.get("queue").ok_or_else(|| {
            "neurosurgery_real_data_review_disposition requires queue".to_string()
        })?;
        let queue = serde_json::from_value::<bioprism_neurosurgery::RealDataReviewQueueReport>(
            queue_value.clone(),
        )
        .map_err(|error| format!("invalid neurosurgical real-data review queue: {error}"))?;
        let decisions = arguments
            .get("decisions")
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new()));
        let decisions = serde_json::from_value::<Vec<RealDataReviewDecision>>(decisions)
            .map_err(|error| format!("invalid real-data review decisions: {error}"))?;
        if decisions.len() > MAX_REAL_DATA_REVIEW_DISPOSITIONS {
            return Err(format!(
                "decisions must contain at most {MAX_REAL_DATA_REVIEW_DISPOSITIONS} items"
            ));
        }
        let request = RealDataReviewDispositionRequest { queue, decisions };
        let report = NeurosurgicalAgent::default()
            .real_data_review_disposition(&request.queue, &request.decisions)
            .map_err(|error| {
                format!("neurosurgical real-data review disposition refused: {error}")
            })?;
        serde_json::to_value(report).map_err(|error| {
            format!("cannot encode neurosurgical real-data review disposition report: {error}")
        })
    }

    /// Compose the validated snapshot's coverage, crosswalk, query, and review queue into one
    /// bounded handoff for a local model or human reviewer.
    pub(super) fn neurosurgery_real_data_evidence_packet(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let data_value = arguments.get("real_glioma_data").ok_or_else(|| {
            "neurosurgery_real_data_evidence_packet requires real_glioma_data".to_string()
        })?;
        let real_glioma_data: RealGliomaBundle = serde_json::from_value(data_value.clone())
            .map_err(|error| format!("invalid neurosurgical real glioma bundle: {error}"))?;
        let query = arguments
            .get("query")
            .map(|value| {
                serde_json::from_value::<RealDataEvidencePacketQuery>(value.clone()).map_err(
                    |error| {
                        format!("invalid neurosurgical real-data evidence-packet query: {error}")
                    },
                )
            })
            .transpose()?
            .unwrap_or_default();
        let report = NeurosurgicalAgent::default()
            .real_data_evidence_packet(&real_glioma_data, &query)
            .map_err(|error| format!("neurosurgical real-data evidence packet refused: {error}"))?;
        serde_json::to_value(report).map_err(|error| {
            format!("cannot encode neurosurgical real-data evidence packet report: {error}")
        })
    }

    /// Compose one deterministic, resumable review wave from the validated real-data packet.
    /// The wave emits only explicit metadata obligations and a final human-review gate.
    pub(super) fn neurosurgery_real_data_autonomous_workflow(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let data_value = arguments.get("real_glioma_data").ok_or_else(|| {
            "neurosurgery_real_data_autonomous_workflow requires real_glioma_data".to_string()
        })?;
        let real_glioma_data: RealGliomaBundle = serde_json::from_value(data_value.clone())
            .map_err(|error| format!("invalid neurosurgical real glioma bundle: {error}"))?;
        let query = arguments
            .get("query")
            .map(|value| {
                serde_json::from_value::<RealDataAutonomousWorkflowQuery>(value.clone()).map_err(
                    |error| {
                        format!(
                            "invalid neurosurgical real-data autonomous-workflow query: {error}"
                        )
                    },
                )
            })
            .transpose()?
            .unwrap_or_default();
        let report = NeurosurgicalAgent::default()
            .real_data_autonomous_workflow(&real_glioma_data, &query)
            .map_err(|error| {
                format!("neurosurgical real-data autonomous workflow refused: {error}")
            })?;
        serde_json::to_value(report).map_err(|error| {
            format!("cannot encode neurosurgical real-data autonomous workflow report: {error}")
        })
    }

    /// Render one validated real-data packet into bounded, source-addressable context for a
    /// caller-owned local model. The renderer never invokes a provider or interprets abstracts.
    pub(super) fn neurosurgery_real_data_reasoning_context(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let data_value = arguments.get("real_glioma_data").ok_or_else(|| {
            "neurosurgery_real_data_reasoning_context requires real_glioma_data".to_string()
        })?;
        let real_glioma_data: RealGliomaBundle = serde_json::from_value(data_value.clone())
            .map_err(|error| format!("invalid neurosurgical real glioma bundle: {error}"))?;
        let query = arguments
            .get("query")
            .map(|value| {
                serde_json::from_value::<RealDataReasoningContextQuery>(value.clone()).map_err(
                    |error| {
                        format!("invalid neurosurgical real-data reasoning-context query: {error}")
                    },
                )
            })
            .transpose()?
            .unwrap_or_default();
        let report = NeurosurgicalAgent::default()
            .real_data_reasoning_context(&real_glioma_data, &query)
            .map_err(|error| {
                format!("neurosurgical real-data reasoning context refused: {error}")
            })?;
        serde_json::to_value(report).map_err(|error| {
            format!("cannot encode neurosurgical real-data reasoning context report: {error}")
        })
    }

    /// Audit a caller-owned local-model/reviewer draft against a freshly composed real-data
    /// packet. This verifies record citations and declared posture only; it never interprets text.
    pub(super) fn neurosurgery_real_data_draft_audit(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let data_value = arguments.get("real_glioma_data").ok_or_else(|| {
            "neurosurgery_real_data_draft_audit requires real_glioma_data".to_string()
        })?;
        let real_glioma_data: RealGliomaBundle = serde_json::from_value(data_value.clone())
            .map_err(|error| format!("invalid neurosurgical real glioma bundle: {error}"))?;
        let query = arguments.get("query").cloned().unwrap_or_else(|| json!({}));
        let claims = arguments
            .get("claims")
            .cloned()
            .ok_or_else(|| "neurosurgery_real_data_draft_audit requires claims".to_string())?;
        let request = serde_json::from_value::<RealDataDraftAuditRequest>(json!({
            "query": query,
            "claims": claims,
        }))
        .map_err(|error| format!("invalid neurosurgical real-data draft: {error}"))?;
        let report = NeurosurgicalAgent::default()
            .real_data_draft_audit(&real_glioma_data, &request)
            .map_err(|error| format!("neurosurgical real-data draft audit refused: {error}"))?;
        serde_json::to_value(report).map_err(|error| {
            format!("cannot encode neurosurgical real-data draft audit report: {error}")
        })
    }
}
