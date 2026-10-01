//! MCP Developer workbench and delivery-evidence handlers.
//!
//! Kept beside the dispatcher so each domain surface can be reviewed independently.

use super::*;

impl Server {
    pub(super) fn developer_platform_status(&self, arguments: &Value) -> Result<Value, String> {
        let include_details = arguments
            .get("include_details")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if max_items == 0 || max_items > 1_000 {
            return Err("max_items must be between 1 and 1000".into());
        }
        let max_items = max_items as usize;
        let walkthroughs = standard_walkthroughs().map_err(|error| error.to_string())?;
        let devplat = DevPlatReport::of(&walkthroughs).map_err(|error| error.to_string())?;
        let cookbook = standard_cookbook().map_err(|error| error.to_string())?;
        let workspace =
            CookbookWorkspace::open(self.root.clone()).map_err(|error| error.to_string())?;
        let verification = cookbook.verify(&workspace);
        let lint = lint_catalogue();
        let exit_audit = devx_audit();
        let contract = workspace_contract();
        let defects = verification.defects();
        let surface_summary = contract
            .iter()
            .take(max_items)
            .map(|surface| {
                json!({
                    "id": surface.id,
                    "owns_count": surface.owns.len(),
                    "invalidates_count": surface.invalidates.len(),
                    "rationale": surface.rationale,
                })
            })
            .collect::<Vec<_>>();
        let mut result = json!({
            "ok": true,
            "root": self.root,
            "detail_mode": if include_details { "full" } else { "summary" },
            "max_items": max_items,
            "devplat": {
                "digest": &devplat.digest,
                "verdict_counts": devplat.verdict_counts,
                "modules_classified": devplat.modules_classified(),
                "implemented_count": devplat.implemented.len(),
                "not_implemented_count": devplat.not_implemented.len(),
                "foreign_subject_count": devplat.foreign_subjects.len(),
                "walkthrough_count": devplat.walkthroughs.len(),
                "guarded_claims": devplat.guarded_claims,
                "unguarded_claims": devplat.unguarded_claims,
            },
            "walkthroughs": walkthroughs.iter().map(|walkthrough| json!({
                "id": walkthrough.id(),
                "goal": walkthrough.goal(),
                "standing": walkthrough.standing(),
                "standing_text": walkthrough.standing().as_str(),
                "steps": walkthrough.steps().len(),
                "claims": walkthrough.claims().len(),
                "guarded_claims": walkthrough.standing().guarded_claims(),
                "unguarded_claims": walkthrough.standing().unguarded_claims(),
                "documents_absent_artifact": walkthrough.documents_absent_artifact(),
                "refuted_claims": walkthrough.refuted_claims().len(),
                "narration_permille": walkthrough.narration_permille(),
            })).collect::<Vec<_>>(),
            "cookbook": {
                "recipes": cookbook.recipes().len(),
                "anti_recipes": cookbook.anti_recipes().len(),
                "crates": cookbook.crates(),
                "enforcing_tests": cookbook.enforcing_tests().len(),
                "quotes": cookbook.quotes().len(),
                "verification": {
                    "clean": verification.is_clean(),
                    "crates_checked": verification.crates.len(),
                    "entry_points_checked": verification.entry_points.len(),
                    "tests_checked": verification.tests.len(),
                    "quotes_checked": verification.quotes.len(),
                    "defect_count": defects.len(),
                    "defects_returned": defects.iter().take(max_items).collect::<Vec<_>>(),
                    "omitted_defects": defects.len().saturating_sub(max_items),
                },
            },
            "developer_contract": {
                "surface_count": contract.len(),
                "surfaces_returned": surface_summary,
                "omitted_surfaces": contract.len().saturating_sub(max_items),
            },
            "diagnostic_catalogue": {
                "clean": lint.is_clean(),
                "checked": lint.checked,
                "errors": lint.errors().len(),
                "warnings": lint.warnings().len(),
                "finding_count": lint.findings.len(),
                "findings_returned": lint.findings.iter().take(max_items).collect::<Vec<_>>(),
                "omitted_findings": lint.findings.len().saturating_sub(max_items),
            },
            "exit_code_audit": {
                "clean": exit_audit.is_clean(),
                "retry_decision_recoverable_from_code_alone": exit_audit.retry_decision_recoverable_from_the_code_alone,
                "divergence_count": exit_audit.divergences.len(),
                "divergences_returned": exit_audit.divergences.iter().take(max_items).collect::<Vec<_>>(),
                "omitted_divergences": exit_audit.divergences.len().saturating_sub(max_items),
            },
            "limitations": [
                "the full Python SDK surface, generated domain clients, gRPC client, hosted consumer CI execution, and authoring UIs remain outside this Rust check; the repository's composite action and bounded TypeScript/Python clients are in-tree, but are not executed by this Rust check",
                "cookbook verification resolves names and tests textually; it does not execute every recipe",
                "developer contracts describe declared blast radius; they do not watch files or run a live debugger",
            ],
        });
        if include_details {
            result["details"] = json!({
                "devplat": devplat,
                "cookbook_verification": verification,
                "developer_contract": contract,
                "diagnostic_findings": lint.findings,
                "exit_code_divergences": exit_audit.divergences,
            });
        }

        Ok(result)
    }

    /// Compose the authoring-studio, notebook, capability-dashboard, and CI-plan contracts.
    ///
    /// The Rust workbench validates the request and returns a digest-bound projection. It is
    /// intentionally not an executor: no notebook kernel, GitHub API, filesystem write, or CI
    /// runner is contacted by this tool.
    pub(super) fn developer_workbench(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode developer workbench input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("developer workbench input exceeds the 20000000-byte safety bound".into());
        }
        let request: WorkbenchRequest = serde_json::from_value(arguments.clone())
            .map_err(|error| format!("invalid developer workbench input: {error}"))?;
        let report = run_workbench(&request)
            .map_err(|error| format!("developer workbench refused: {error}"))?;
        let mut output = serde_json::to_value(report)
            .map_err(|error| format!("cannot encode developer workbench report: {error}"))?;
        output["ok"] = json!(true);
        output["workflow"] = json!("developer_workbench");
        output["workbench_schema_version"] = json!(WORKBENCH_SCHEMA_VERSION);
        Ok(output)
    }

    /// Verify a retained authoring/notebook report without executing cells or contacting CI.
    pub(super) fn developer_workbench_verify(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments).map_err(|error| {
            format!("cannot encode developer workbench verification input: {error}")
        })?;
        if encoded.len() > 20_000_000 {
            return Err(
                "developer workbench verification input exceeds the 20000000-byte safety bound"
                    .into(),
            );
        }
        let request: WorkbenchVerificationRequest = serde_json::from_value(arguments.clone())
            .map_err(|error| format!("invalid developer workbench verification input: {error}"))?;
        let report = verify_workbench(&request)
            .map_err(|error| format!("developer workbench verification refused: {error}"))?;
        let mut output = serde_json::to_value(report).map_err(|error| {
            format!("cannot encode developer workbench verification report: {error}")
        })?;
        output["ok"] = json!(true);
        output["workflow"] = json!("developer_workbench_verify");
        output["workbench_verify_schema_version"] =
            json!(bioprism_devplat::WORKBENCH_VERIFY_SCHEMA_VERSION);
        Ok(output)
    }

    /// Retain a structurally valid developer workbench report in the bounded shared registry.
    ///
    /// This is an explicit handoff: generating a report never silently mutates retention state.
    /// The import kernel strips transport metadata and revalidates the typed report before the
    /// canonical report digest becomes queryable.
    pub(super) fn developer_workbench_import(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode developer workbench import input: {error}"))?;
        if encoded.len() > bioprism_devplat::MAX_WORKBENCH_REGISTRY_BYTES {
            return Err(format!(
                "developer workbench import input exceeds the {}-byte safety bound",
                bioprism_devplat::MAX_WORKBENCH_REGISTRY_BYTES
            ));
        }
        let report = arguments.get("report").ok_or("report is required")?;
        self.workbench_registry
            .lock()
            .map_err(|_| "workbench registry lock is poisoned".to_string())?
            .import(report)
            .map_err(|error| format!("developer workbench import refused: {error}"))
    }

    /// Query retained workbench reports by session and dashboard posture without executing work.
    pub(super) fn developer_workbench_query(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode developer workbench query input: {error}"))?;
        if encoded.len() > 1_000_000 {
            return Err(
                "developer workbench query input exceeds the 1000000-byte safety bound".into(),
            );
        }
        let optional_string = |name: &str| -> Result<Option<&str>, String> {
            arguments
                .get(name)
                .map(|value| {
                    value
                        .as_str()
                        .ok_or_else(|| format!("{name} must be a string"))
                })
                .transpose()
        };
        let session_digest = optional_string("session_digest")?;
        let domain = optional_string("domain")?;
        let capability = optional_string("capability")?;
        let state = optional_string("state")?;
        let after = optional_string("after")?;
        let release_ready = arguments
            .get("release_ready")
            .map(|value| value.as_bool().ok_or("release_ready must be a boolean"))
            .transpose()?;
        let max_items = arguments
            .get("max_items")
            .map(|value| {
                value
                    .as_u64()
                    .ok_or_else(|| "max_items must be an integer".to_string())
                    .and_then(|number| {
                        usize::try_from(number).map_err(|_| "max_items is too large".to_string())
                    })
            })
            .transpose()?
            .unwrap_or(100);
        let include_reports = arguments
            .get("include_reports")
            .map(|value| value.as_bool().ok_or("include_reports must be a boolean"))
            .transpose()?
            .unwrap_or(false);
        self.workbench_registry
            .lock()
            .map_err(|_| "workbench registry lock is poisoned".to_string())?
            .query(
                session_digest,
                domain,
                capability,
                state,
                release_ready,
                after,
                max_items,
                include_reports,
            )
            .map_err(|error| format!("developer workbench query refused: {error}"))
    }

    /// Fetch one retained workbench report by its canonical content hash.
    pub(super) fn developer_workbench_get(&self, arguments: &Value) -> Result<Value, String> {
        let digest = arguments
            .get("workbench_report_digest")
            .and_then(Value::as_str)
            .ok_or("workbench_report_digest is required and must be a content hash")?;
        bioprism_ids::ContentHash::parse(digest.to_string())
            .map_err(|error| format!("workbench_report_digest is invalid: {error}"))?;
        self.workbench_registry
            .lock()
            .map_err(|_| "workbench registry lock is poisoned".to_string())?
            .get_response(digest)
            .map_err(|error| format!("developer workbench get refused: {error}"))
    }

    pub(super) fn developer_delivery_audit(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot measure developer-delivery input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("developer-delivery input exceeds the 20000000-byte safety bound".into());
        }

        let empty = json!({});
        let platform =
            self.developer_platform_status(arguments.get("platform").unwrap_or(&empty))?;
        let repository = self.repository_catalog(arguments.get("repository").unwrap_or(&empty))?;
        let repository_impact = match arguments.get("repository_impact") {
            Some(raw) => Some(self.repository_impact(raw)?),
            None => None,
        };
        let sdk = match arguments.get("sdk") {
            Some(raw) => Some(self.sdk_registry_check(raw)?),
            None => None,
        };
        let conformance = match arguments.get("conformance") {
            Some(raw) => Some(self.conformance_run(raw)?),
            None => None,
        };
        let provider = match arguments.get("provider") {
            Some(raw) => Some(self.provider_capability_gate(raw)?),
            None => None,
        };
        let governance = match arguments.get("governance") {
            Some(raw) => Some(self.governance_schema_check(raw)?),
            None => None,
        };
        let release = match arguments.get("release") {
            Some(raw) => Some(self.release_audit(raw)?),
            None => None,
        };
        let ci_evidence = match arguments.get("ci_evidence") {
            Some(raw) => Some(self.ci_execution_evidence_audit(raw)?),
            None => None,
        };
        let ci_provider_normalization = match arguments.get("ci_provider") {
            Some(_) if arguments.get("ci_evidence").is_some()
                || arguments.get("ci_provider_evidence").is_some() =>
            {
                return Err(
                    "ci_provider, ci_provider_evidence, and ci_evidence are mutually exclusive; supply one evidence source"
                        .into(),
                )
            }
            Some(raw) => {
                let normalized = self.ci_provider_normalize(raw)?;
                let ci = raw
                    .get("ci")
                    .cloned()
                    .ok_or("ci_provider requires the canonical ci request")?;
                let evidence = normalized
                    .get("evidence")
                    .cloned()
                    .ok_or("ci_provider normalization returned no evidence envelope")?;
                let audit = self.ci_execution_evidence_audit(&json!({
                    "ci": ci,
                    "evidence": evidence,
                }))?;
                Some((normalized, audit))
            }
            None => None,
        };
        let ci_provider_evidence = match arguments.get("ci_provider_evidence") {
            Some(_) if arguments.get("ci_evidence").is_some() => {
                return Err(
                    "ci_provider_evidence and ci_evidence are mutually exclusive; supply one evidence source"
                        .into(),
                )
            }
            Some(_) if arguments.get("ci_provider").is_some() => {
                return Err(
                    "ci_provider_evidence and ci_provider are mutually exclusive; supply one evidence source"
                        .into(),
                )
            }
            Some(raw) => {
                let provider_evidence = self.ci_provider_evidence_audit(raw)?;
                let ci = raw
                    .get("ci")
                    .cloned()
                    .ok_or("ci_provider_evidence requires the canonical ci request")?;
                let evidence = provider_evidence
                    .get("evidence")
                    .cloned()
                    .ok_or("ci_provider_evidence returned no canonical CI evidence envelope")?;
                let audit = self.ci_execution_evidence_audit(&json!({
                    "ci": ci,
                    "evidence": evidence,
                }))?;
                Some((provider_evidence, audit))
            }
            None => None,
        };
        let ci_evidence = ci_evidence
            .or_else(|| {
                ci_provider_normalization
                    .as_ref()
                    .map(|(_, audit)| audit.clone())
            })
            .or_else(|| {
                ci_provider_evidence
                    .as_ref()
                    .map(|(_, audit)| audit.clone())
            });
        let execution_provenance = match arguments.get("execution_provenance") {
            Some(raw) => Some(self.execution_provenance_audit(raw)?),
            None => None,
        };

        let platform_ok = platform.get("ok").and_then(Value::as_bool).unwrap_or(false);
        let platform_checks_clean = platform_ok
            && platform
                .get("cookbook")
                .and_then(|value| value.get("verification"))
                .and_then(|value| value.get("clean"))
                .and_then(Value::as_bool)
                .unwrap_or(false)
            && platform
                .get("diagnostic_catalogue")
                .and_then(|value| value.get("clean"))
                .and_then(Value::as_bool)
                .unwrap_or(false)
            && platform
                .get("exit_code_audit")
                .and_then(|value| value.get("clean"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
        let unguarded_claims = platform
            .get("devplat")
            .and_then(|value| value.get("unguarded_claims"))
            .and_then(Value::as_u64)
            .unwrap_or(u64::MAX);
        let platform_ready = platform_checks_clean;
        let developer_claims_ready = platform_ready && unguarded_claims == 0;

        let repository_ready = repository
            .get("ok")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && repository
                .get("unresolved_links")
                .and_then(Value::as_u64)
                .unwrap_or(1)
                == 0
            && repository
                .get("out_of_corpus_links")
                .and_then(Value::as_u64)
                .unwrap_or(1)
                == 0
            && repository
                .get("unreadable_front_matter")
                .and_then(Value::as_u64)
                .unwrap_or(1)
                == 0
            && repository
                .get("lint")
                .and_then(|value| value.get("errors"))
                .and_then(Value::as_u64)
                .unwrap_or(1)
                == 0;
        let repository_impact_ready = repository_impact
            .as_ref()
            .map(|value| {
                value.get("ok").and_then(Value::as_bool).unwrap_or(false)
                    && value
                        .get("scan")
                        .and_then(|scan| scan.get("unresolved_links"))
                        .and_then(Value::as_u64)
                        .unwrap_or(1)
                        == 0
                    && value
                        .get("scan")
                        .and_then(|scan| scan.get("out_of_corpus_links"))
                        .and_then(Value::as_u64)
                        .unwrap_or(1)
                        == 0
                    && value
                        .get("scan")
                        .and_then(|scan| scan.get("unreadable_front_matter"))
                        .and_then(Value::as_u64)
                        .unwrap_or(1)
                        == 0
            })
            .unwrap_or(false);

        let sdk_ready = sdk
            .as_ref()
            .map(|value| {
                value.get("ok").and_then(Value::as_bool).unwrap_or(false)
                    && value
                        .get("manifest_count")
                        .and_then(Value::as_u64)
                        .unwrap_or(0)
                        > 0
                    && value
                        .get("registry")
                        .and_then(|registry| registry.get("registration_count"))
                        .and_then(Value::as_u64)
                        == value.get("manifest_count").and_then(Value::as_u64)
            })
            .unwrap_or(false);
        let conformance_ready = conformance
            .as_ref()
            .map(|value| {
                value.get("ok").and_then(Value::as_bool).unwrap_or(false)
                    && value
                        .get("release_decision")
                        .and_then(|decision| decision.get("decision"))
                        .and_then(Value::as_str)
                        == Some("release")
            })
            .unwrap_or(false);
        let provider_ready = provider
            .as_ref()
            .map(|value| {
                value.get("ok").and_then(Value::as_bool).unwrap_or(false)
                    && value
                        .get("gate")
                        .and_then(|gate| gate.get("outcome"))
                        .and_then(Value::as_str)
                        == Some("cleared")
            })
            .unwrap_or(false);
        let governance_ready = governance
            .as_ref()
            .map(|value| {
                value.get("ok").and_then(Value::as_bool).unwrap_or(false)
                    && value.get("mode").and_then(Value::as_str) == Some("document")
                    && value
                        .get("conforms")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                    && value
                        .get("is_clean")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
            })
            .unwrap_or(false);
        let release_ready = release
            .as_ref()
            .map(|value| {
                value.get("ok").and_then(Value::as_bool).unwrap_or(false)
                    && value
                        .get("release_ready")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
            })
            .unwrap_or(false);
        let ci_evidence_ready = ci_evidence
            .as_ref()
            .map(|value| {
                value.get("ok").and_then(Value::as_bool).unwrap_or(false)
                    && value
                        .get("ci_evidence_ready")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
            })
            .unwrap_or(false);
        let ci_provider_evidence_ready = ci_provider_evidence
            .as_ref()
            .map(|(value, _)| {
                value.get("ok").and_then(Value::as_bool).unwrap_or(false)
                    && value
                        .get("conformance_ready")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
            })
            .unwrap_or(false);
        let execution_provenance_ready = execution_provenance
            .as_ref()
            .map(|value| {
                value.get("ok").and_then(Value::as_bool).unwrap_or(false)
                    && value
                        .get("provenance_ready")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
            })
            .unwrap_or(false);
        let local_delivery_ready = platform_ready && repository_ready;

        let mut release_request = if let Some(raw_request) = arguments.get("release_request") {
            let request = raw_request
                .as_object()
                .ok_or("release_request must be an object")?;
            let request_id = request
                .get("id")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or("release_request.id must be a non-empty string")?;
            let raw_targets = request
                .get("targets")
                .and_then(Value::as_array)
                .ok_or("release_request.targets must be an array")?;
            if raw_targets.is_empty() || raw_targets.len() > 16 {
                return Err("release_request.targets must contain between 1 and 16 entries".into());
            }

            let mut seen = BTreeSet::new();
            let mut target_rows = Vec::with_capacity(raw_targets.len());
            for raw_target in raw_targets {
                let target = raw_target
                    .as_str()
                    .ok_or("release_request.targets must contain strings")?;
                if !seen.insert(target.to_string()) {
                    return Err(format!(
                        "release_request.targets contains duplicate {target:?}"
                    ));
                }
                let (available, eligible, blockers, notes) = match target {
                    "local_delivery" => (
                        true,
                        local_delivery_ready,
                        if local_delivery_ready {
                            vec![]
                        } else {
                            let mut blockers = Vec::new();
                            if !platform_ready {
                                blockers.push("developer_platform_not_clean".to_string());
                            }
                            if !repository_ready {
                                blockers.push("repository_scope_not_clean".to_string());
                            }
                            blockers
                        },
                        vec![
                            "local delivery binds platform contract health to repository graph health"
                                .to_string(),
                        ],
                    ),
                    "developer_platform" => (
                        true,
                        platform_ready,
                        if platform_ready {
                            vec![]
                        } else {
                            vec!["developer_platform_checks_not_clean".to_string()]
                        },
                        vec![
                            "foreign SDK, CI, gRPC, and UI artifacts remain explicit limitations; the bounded REST/event gateway is separately reported"
                                .to_string(),
                        ],
                    ),
                    "developer_claims" => (
                        true,
                        developer_claims_ready,
                        if developer_claims_ready {
                            vec![]
                        } else if !platform_ready {
                            vec!["developer_platform_checks_not_clean".to_string()]
                        } else {
                            vec!["unguarded_developer_claims_present".to_string()]
                        },
                        vec![
                            "unguarded walkthrough claims are never converted into release evidence"
                                .to_string(),
                        ],
                    ),
                    "repository_scope" => (
                        true,
                        repository_ready,
                        if repository_ready {
                            vec![]
                        } else {
                            vec!["repository_graph_or_lint_not_clean".to_string()]
                        },
                        vec![
                            "scope readiness requires a bounded scan with no unresolved or unreadable graph health findings"
                                .to_string(),
                        ],
                    ),
                    "repository_impact" => (
                        repository_impact.is_some(),
                        repository_impact_ready,
                        if repository_impact.is_none() {
                            vec!["repository_impact_arguments_missing".to_string()]
                        } else if repository_impact_ready {
                            vec![]
                        } else {
                            vec!["repository_impact_scan_not_clean".to_string()]
                        },
                        vec![
                            "impact is conservative incoming-dependent closure, not a semantic diff"
                                .to_string(),
                        ],
                    ),
                    "sdk_admission" => (
                        sdk.is_some(),
                        sdk_ready,
                        if sdk.is_none() {
                            vec!["sdk_arguments_missing".to_string()]
                        } else if sdk_ready {
                            vec![]
                        } else {
                            vec!["sdk_registry_admission_not_clean".to_string()]
                        },
                        vec![
                            "registration success does not imply trust, isolation, signature verification, or runtime conformance"
                                .to_string(),
                        ],
                    ),
                    "conformance" => (
                        conformance.is_some(),
                        conformance_ready,
                        if conformance.is_none() {
                            vec!["conformance_arguments_missing".to_string()]
                        } else if conformance_ready {
                            vec![]
                        } else {
                            vec!["conformance_release_decision_not_release".to_string()]
                        },
                        vec![
                            "fixture verification and the declared equal-engineering baseline remain part of the conformance evidence"
                                .to_string(),
                        ],
                    ),
                    "provider_capability" => (
                        provider.is_some(),
                        provider_ready,
                        if provider.is_none() {
                            vec!["provider_arguments_missing".to_string()]
                        } else if provider_ready {
                            vec![]
                        } else {
                            vec!["provider_capability_gate_not_cleared".to_string()]
                        },
                        vec![
                            "untested and failed required capabilities both block; measurements are not pass/fail claims"
                                .to_string(),
                        ],
                    ),
                    "governance_schema" => (
                        governance.is_some(),
                        governance_ready,
                        if governance.is_none() {
                            vec!["governance_arguments_missing".to_string()]
                        } else if governance_ready {
                            vec![]
                        } else {
                            vec!["governance_document_not_clean".to_string()]
                        },
                        vec![
                            "schema catalog presence is not treated as document conformance"
                                .to_string(),
                        ],
                    ),
                    "release" => (
                        release.is_some(),
                        release_ready,
                        if release.is_none() {
                            vec!["release_arguments_missing".to_string()]
                        } else if release_ready {
                            vec![]
                        } else {
                            vec!["release_audit_not_ready".to_string()]
                        },
                        vec![
                            "release readiness is the conjunction of named required checks and never a score"
                                .to_string(),
                        ],
                    ),
                    "ci_execution_evidence" => (
                        ci_evidence.is_some(),
                        ci_evidence_ready,
                        if ci_evidence.is_none() {
                            vec!["ci_evidence_arguments_missing".to_string()]
                        } else if ci_evidence_ready {
                            vec![]
                        } else {
                            vec!["ci_execution_evidence_not_ready".to_string()]
                        },
                        vec![
                            "CI evidence readiness requires an exact regenerated plan digest, complete passing checks, and a successful conclusion; it remains structural-only"
                                .to_string(),
                        ],
                    ),
                    "ci_provider_evidence" => (
                        ci_provider_evidence.is_some(),
                        ci_provider_evidence_ready,
                        if ci_provider_evidence.is_none() {
                            vec!["ci_provider_evidence_arguments_missing".to_string()]
                        } else if ci_provider_evidence_ready {
                            vec![]
                        } else {
                            vec!["ci_provider_evidence_not_conformant".to_string()]
                        },
                        vec![
                            "provider artifact, log, and attestation rows are bound and digest-addressed structurally; remote bytes, provider authority, and signatures remain outside this route"
                                .to_string(),
                        ],
                    ),
                    "execution_provenance" => (
                        execution_provenance.is_some(),
                        execution_provenance_ready,
                        if execution_provenance.is_none() {
                            vec!["execution_provenance_arguments_missing".to_string()]
                        } else if execution_provenance_ready {
                            vec![]
                        } else {
                            vec!["execution_provenance_not_ready".to_string()]
                        },
                        vec![
                            "mission provenance readiness requires a complete, identity-consistent structural trace and passing required delegated checks; it never replays the mission"
                                .to_string(),
                        ],
                    ),
                    other => {
                        return Err(format!(
                            "unknown release target {other:?}; choose local_delivery, developer_platform, developer_claims, repository_scope, repository_impact, sdk_admission, conformance, provider_capability, governance_schema, release, ci_execution_evidence, ci_provider_evidence, or execution_provenance"
                        ));
                    }
                };
                target_rows.push(json!({
                    "target": target,
                    "available": available,
                    "eligible": eligible,
                    "blockers": blockers,
                    "notes": notes,
                }));
            }
            let ready = target_rows.iter().all(|row| {
                row.get("eligible")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            });
            json!({
                "present": true,
                "id": request_id,
                "targets": target_rows,
                "ready": ready,
                "fail_closed": !ready,
                "no_implicit_release": true,
            })
        } else {
            json!({
                "present": false,
                "ready": false,
                "reason": "release_request is required to make a delivery readiness claim",
                "no_implicit_release": true,
            })
        };
        release_request["available_target_count"] = json!(13);

        let foreign_subject_count = platform
            .get("devplat")
            .and_then(|value| value.get("foreign_subject_count"))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        Ok(json!({
            "ok": true,
            "workflow": "developer_delivery_audit",
            "platform": platform,
            "repository": repository,
            "repository_impact": repository_impact,
            "sdk": sdk,
            "conformance": conformance,
            "provider": provider,
            "governance": governance,
            "release": release,
            "ci_evidence": ci_evidence,
            "ci_provider_normalization": ci_provider_normalization
                .as_ref()
                .map(|(normalized, _)| normalized.clone()),
            "ci_provider_evidence": ci_provider_evidence
                .as_ref()
                .map(|(evidence, _)| evidence.clone()),
            "execution_provenance": execution_provenance,
            "readiness": {
                "platform_checks_clean": platform_ready,
                "unguarded_claims": unguarded_claims,
                "developer_claims_ready": developer_claims_ready,
                "repository_scope_clean": repository_ready,
                "repository_impact_clean": repository_impact_ready,
                "sdk_admission_clean": sdk_ready,
                "conformance_release": conformance_ready,
                "provider_capability_gate_cleared": provider_ready,
                "governance_document_clean": governance_ready,
                "release_audit_ready": release_ready,
                "ci_execution_evidence_ready": ci_evidence_ready,
                "ci_provider_evidence_ready": ci_provider_evidence_ready,
                "execution_provenance_ready": execution_provenance_ready,
                "local_delivery_ready": local_delivery_ready,
            },
            "external_surface_posture": {
                "foreign_subject_count": foreign_subject_count,
                "foreign_artifacts_present": foreign_subject_count > 0,
                "foreign_artifacts_are_not_inferred": true,
                "local_integration_foundations": [
                    {
                        "artifact": "python/prism_sdk",
                        "kind": "dependency_free_mcp_stdio_client",
                        "scope": "transport_lifecycle_result_and_workflow_facade",
                        "full_blueprint_sdk_verified": false
                    },
                    {
                        "artifact": ".github/actions/autonomous-run",
                        "kind": "approval_bounded_composite_action",
                        "scope": "caller_owned_mcp_provider_and_explicit_execution_approvals",
                        "local_ci_exercise": true,
                        "hosted_consumer_execution_verified": false
                    }
                ],
                "unverified_surface_families": [
                    "python_biological_adapters_and_statistics",
                    "python_benchmark_authoring_and_notebook_ergonomics",
                    "typescript_sdk",
                    "rest_grpc_clients",
                    "event_streams_and_webhooks",
                    "hosted_github_action_and_consumer_ci_execution",
                    "authoring_studio_ui",
                ],
            },
            "release_request": release_request,
            "guarantees": [
                "local platform and repository health are composed without turning foreign artifacts into implemented claims",
                "SDK admission, conformance, provider capability, governance, impact, and release evidence remain independently inspectable",
                "missing optional evidence blocks only the explicit target that depends on it",
                "a readiness claim is emitted only for explicit requested targets; no score, count, or partial green result creates an implicit release",
                "all delegated checks remain bounded and side-effect free; no package is published, signed, deployed, fetched, or executed by this workflow",
            ],
            "limitations": [
                "the workflow does not implement the full Python SDK surface, generated domain clients, biological Python adapters, gRPC clients, hosted CI runners, arbitrary consumer-workflow execution, or authoring UIs; the in-repository composite action is tested locally, while REST/event access is provided by the bounded bioprism-api gateway and typescript/ client",
                "repository readiness is graph and lint health, not proof that every prose requirement is implemented",
                "SDK registry admission validates serialized declarations but does not dynamically load or sandbox plugins",
                "conformance and provider outputs are evidence for delivery review, not clinical, scientific, security, or production approval",
            ],
        }))
    }

    pub(super) fn developer_delivery_receipt(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot measure developer-delivery receipt input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err(
                "developer-delivery receipt input exceeds the 20000000-byte safety bound".into(),
            );
        }
        let receipt_id = arguments
            .get("receipt_id")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or("receipt_id is required and must be a non-empty string")?;
        let delivery_arguments = arguments
            .get("delivery")
            .ok_or("delivery is required and must contain developer_delivery_audit arguments")?;
        if !delivery_arguments.is_object() {
            return Err("delivery must be an object".into());
        }
        let delivery = self.developer_delivery_audit(delivery_arguments)?;
        let request = DeliveryReceiptRequest {
            receipt_id: receipt_id.into(),
            delivery: delivery.clone(),
        };
        let receipt = build_delivery_receipt(&request)?;
        let mut output = serde_json::to_value(&receipt)
            .map_err(|error| format!("cannot encode developer-delivery receipt: {error}"))?;
        output["ok"] = json!(true);
        output["workflow"] = json!("developer_delivery_receipt");
        output["valid"] = json!(receipt.structurally_valid);
        output["receipt_ready"] = json!(receipt.release_candidate);
        output["delivery"] = delivery;
        Ok(output)
    }

    pub(super) fn developer_delivery_receipt_verify(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments).map_err(|error| {
            format!("cannot measure developer-delivery receipt verification input: {error}")
        })?;
        if encoded.len() > 20_000_000 {
            return Err("developer-delivery receipt verification input exceeds the 20000000-byte safety bound".into());
        }
        let receipt = arguments
            .get("receipt")
            .cloned()
            .ok_or("receipt is required and must be a serialized developer delivery receipt")?;
        let delivery = arguments
            .get("delivery")
            .cloned()
            .ok_or("delivery is required and must be the completed developer delivery audit")?;
        let verification =
            verify_delivery_receipt(&DeliveryReceiptVerificationRequest { receipt, delivery })?;
        let mut output = serde_json::to_value(&verification).map_err(|error| {
            format!("cannot encode developer-delivery receipt verification: {error}")
        })?;
        output["ok"] = json!(true);
        output["workflow"] = json!("developer_delivery_receipt_verify");
        output["verified"] = json!(verification.valid);
        Ok(output)
    }

    pub(super) fn provider_capability_gate(&self, arguments: &Value) -> Result<Value, String> {
        let raw_card = arguments
            .get("card")
            .cloned()
            .ok_or("card is required and must be a serialized CapabilityCard")?;
        let card_bytes = serde_json::to_vec(&raw_card).map_err(|error| error.to_string())?;
        if card_bytes.len() > 5_000_000 {
            return Err("capability card exceeds the 5000000-byte safety bound".into());
        }
        let card: SweepCapabilityCard = serde_json::from_value(raw_card)
            .map_err(|error| format!("invalid provider capability card: {error}"))?;
        let raw_required = arguments
            .get("required")
            .cloned()
            .ok_or("required is required and must contain capability check names")?;
        let required_items = raw_required.as_array().ok_or("required must be an array")?;
        if required_items.is_empty() || required_items.len() > 17 {
            return Err("required must contain between 1 and 17 checks".into());
        }
        let required: Vec<SweepCheck> = serde_json::from_value(raw_required)
            .map_err(|error| format!("invalid required capability checks: {error}"))?;
        if required.iter().any(|check| !check.is_pass_fail()) {
            return Err(
                "performance checks are measurements and cannot be used as pass/fail requirements"
                    .into(),
            );
        }
        let include_card = arguments
            .get("include_card")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let differential = match arguments.get("other_card") {
            None => None,
            Some(raw_other) => {
                let other: SweepCapabilityCard = serde_json::from_value(raw_other.clone())
                    .map_err(|error| format!("invalid other provider capability card: {error}"))?;
                Some(sweep_differential(&card, &other))
            }
        };
        let required_states = required
            .iter()
            .map(|check| (format!("{check:?}"), json!(card.state(*check))))
            .collect::<serde_json::Map<String, Value>>();
        let gate = sweep_gate(&card, &required);

        Ok(json!({
            "ok": true,
            "provider": serde_json::to_value(&card)
                .ok()
                .and_then(|value| value.get("provider").cloned()),
            "required": required,
            "required_states": required_states,
            "gate": gate,
            "claims": card.claims(),
            "measurement_count": card.measurements().len(),
            "differential": differential,
            "card": include_card.then_some(card),
            "guarantees": [
                "untested and failed capabilities both block a required claim",
                "performance measurements cannot be turned into pass/fail claims without an authorized threshold",
                "a differential with an untested side remains indeterminate, not agreement",
                "the gate reports provider evidence and does not execute runtime tests",
            ],
        }))
    }

    pub(super) fn sdk_registry_check(&self, arguments: &Value) -> Result<Value, String> {
        let raw_manifests = arguments
            .get("manifests")
            .and_then(Value::as_array)
            .ok_or("manifests is required and must be an array of PluginManifest values")?;
        if raw_manifests.is_empty() || raw_manifests.len() > 256 {
            return Err("manifests must contain between 1 and 256 entries".into());
        }
        let encoded = serde_json::to_vec(&json!({
            "manifests": raw_manifests,
            "policy": arguments.get("policy"),
        }))
        .map_err(|error| format!("cannot measure SDK registry envelope: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("SDK registry input exceeds the 20000000-byte safety bound".into());
        }
        let policy = arguments
            .get("policy")
            .cloned()
            .map(serde_json::from_value::<SdkRegistryPolicy>)
            .transpose()
            .map_err(|error| format!("invalid registry policy: {error}"))?
            .unwrap_or_default();

        let mut manifests = Vec::with_capacity(raw_manifests.len());
        let mut rows = Vec::with_capacity(raw_manifests.len());
        let mut invalid = false;
        for (index, raw) in raw_manifests.iter().enumerate() {
            let manifest: PluginManifest = match serde_json::from_value(raw.clone()) {
                Ok(manifest) => manifest,
                Err(error) => {
                    invalid = true;
                    rows.push(json!({
                        "index": index,
                        "valid": false,
                        "refusal": format!("invalid plugin manifest: {error}"),
                    }));
                    continue;
                }
            };
            let validation = manifest.validate();
            let digest = manifest.digest().ok().map(|value| value.to_string());
            let core_digest = manifest.core_digest().ok().map(|value| value.to_string());
            let trust = manifest.trust().ok();
            if validation.is_err() {
                invalid = true;
            }
            rows.push(json!({
                "index": index,
                "id": manifest.id(),
                "valid": validation.is_ok(),
                "validation_error": validation.as_ref().err().map(ToString::to_string),
                "digest": digest,
                "core_digest": core_digest,
                "capability_kinds": manifest.capability_kinds().into_iter().map(|kind| format!("{kind:?}")).collect::<Vec<_>>(),
                "trust": trust,
            }));
            if validation.is_ok() {
                manifests.push(manifest);
            }
        }
        if invalid {
            return Ok(json!({
                "ok": false,
                "stage": "manifest_validation",
                "manifests": rows,
                "registry": Value::Null,
                "fail_closed": true,
                "guarantees": [
                    "registration never repairs or infers a malformed manifest",
                    "digest and trust metadata are diagnostic claims, not behavioural verification",
                    "no partial registry is returned when one manifest is invalid",
                ],
            }));
        }

        let registry = match PluginRegistry::from_manifests(policy, manifests) {
            Ok(registry) => registry,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "stage": "registry_registration",
                    "manifests": rows,
                    "registry": Value::Null,
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "guarantees": [
                        "schema negotiation, effect policy, duplicate identities, and capability conflicts remain refusals",
                        "registration order is normalized before conflict reporting",
                        "a refused set is never projected as a resolved capability map",
                    ],
                }));
            }
        };
        let resolution = registry
            .resolution()
            .into_iter()
            .map(|(kind, plugin)| (format!("{kind:?}"), json!(plugin)))
            .collect::<BTreeMap<_, _>>();
        let registrations = registry
            .registrations()
            .map(|registration| {
                json!({
                    "id": registration.id(),
                    "digest": registration.digest,
                    "core_digest": registration.core_digest,
                    "negotiated": registration.negotiated,
                    "trust": registration.trust,
                    "load_bearing_selectable": registration.is_selectable_for_load_bearing(registry.policy().load_bearing_floor),
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({
            "ok": true,
            "manifest_count": rows.len(),
            "manifests": rows,
            "registry": {
                "registration_count": registry.len(),
                "resolution": resolution,
                "registrations": registrations,
                "policy": registry.policy(),
            },
            "conformance_note": conformance_note(),
            "guarantees": [
                "manifest declarations are validated before registry admission",
                "resolved capabilities are deterministic only after the full set registers cleanly",
                "trust and load-bearing selection remain separate from registration success",
                "no dynamic loading, signature verification, isolation, network access, or conformance execution is implied",
            ],
        }))
    }

    pub(super) fn governance_schema_check(&self, arguments: &Value) -> Result<Value, String> {
        let requested = arguments.get("schema").and_then(Value::as_str);
        if arguments.get("schema").is_some() && requested.is_none() {
            return Err("schema must be a schema id string".into());
        }
        let document_relative = arguments.get("document").and_then(Value::as_str);
        let document = match document_relative {
            None => None,
            Some(relative) => {
                let path = self.resolve(relative)?;
                if path.is_dir() {
                    return Err(
                        "governance_schema_check requires a JSON document, not a directory".into(),
                    );
                }
                Some((relative, self.read_json(&path)?))
            }
        };
        let descriptors = known_schemas::all();
        let descriptor_json = |descriptor: &bioprism_governance::SchemaDescriptor| {
            json!({
                "id": descriptor.id,
                "mode": descriptor.mode,
                "field_count": descriptor.fields().len(),
                "fields": descriptor.fields(),
                "paths": descriptor.paths(),
                "hashed_paths": descriptor.hashed_paths(),
            })
        };

        if document.is_none() {
            let selected = descriptors
                .iter()
                .filter(|descriptor| requested.is_none_or(|id| descriptor.id.to_string() == id))
                .map(descriptor_json)
                .collect::<Vec<_>>();
            if selected.is_empty() {
                return Err(format!(
                    "unknown schema {:?}; known schemas are {:?}",
                    requested.unwrap_or(""),
                    descriptors
                        .iter()
                        .map(|descriptor| descriptor.id.to_string())
                        .collect::<Vec<_>>()
                ));
            }
            return Ok(json!({
                "ok": true,
                "mode": "catalog",
                "schema_count": selected.len(),
                "schemas": selected,
                "unknown_fields_are_checked_by_declared_mode": true,
            }));
        }

        let (document_relative, document) = document.expect("document was checked above");
        let selected_id = requested.map(str::to_string).or_else(|| {
            document
                .get("schema_version")
                .and_then(Value::as_str)
                .map(str::to_string)
        });
        let selected_id =
            selected_id.ok_or("schema is required when the document has no schema_version")?;
        let descriptor = descriptors
            .iter()
            .find(|descriptor| descriptor.id.to_string() == selected_id)
            .ok_or_else(|| {
                format!(
                    "unknown schema {selected_id:?}; known schemas are {:?}",
                    descriptors
                        .iter()
                        .map(|descriptor| descriptor.id.to_string())
                        .collect::<Vec<_>>()
                )
            })?;
        let check = descriptor.check_document(&document);
        let metadata = descriptor_json(descriptor);
        Ok(json!({
            "ok": true,
            "mode": "document",
            "document": document_relative,
            "schema": metadata,
            "check": check,
            "conforms": check.conforms(),
            "is_clean": check.is_clean(),
            "unknown_fields_are_not_conformance_failures": true,
        }))
    }
}
