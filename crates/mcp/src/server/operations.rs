//! Local operational readiness, capacity, and research-CI projections.

use super::*;

impl Server {
    pub(super) fn operations_catalog(&self, arguments: &Value) -> Result<Value, String> {
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

        let local = reference_local().map_err(|error| error.to_string())?;
        let team = reference_team().map_err(|error| error.to_string())?;
        let parity = topology_parity(&local, &team);
        let topology = |topology: &bioprism_dataops::StorageTopology| {
            json!({
                "deployment": topology.deployment(),
                "technologies": topology.technologies(),
                "classes": DataClass::ALL.into_iter().map(|class| json!({
                    "class": class,
                    "name": class.name(),
                    "store": topology.store_for(class),
                    "promises": topology.promises(class),
                    "holds_immutable_evidence": class.holds_immutable_evidence(),
                })).collect::<Vec<_>>(),
            })
        };

        let service_entries = service_audit();
        let service_summary = AuditSummary::of(&service_entries);
        let service_rows = service_entries
            .iter()
            .take(max_items)
            .map(|entry| {
                json!({
                    "module_id": entry.module_id,
                    "title": entry.title,
                    "contract": entry.contract,
                    "crates": entry.crates,
                    "verdict": entry.verdict,
                    "divergence_count": entry.divergence_count(),
                    "divergences": entry.divergences.iter().take(max_items).collect::<Vec<_>>(),
                    "omitted_divergences": entry.divergences.len().saturating_sub(max_items),
                })
            })
            .collect::<Vec<_>>();
        let tenant_patterns = [
            TenantPattern::SharedControl,
            TenantPattern::DedicatedInstallation,
            TenantPattern::AirGappedRegistry,
            TenantPattern::HybridPublicMetadata,
        ]
        .into_iter()
        .map(|pattern| {
            json!({
                "pattern": pattern,
                "name": pattern.name(),
            })
        })
        .collect::<Vec<_>>();

        let mut result = json!({
            "ok": true,
            "detail_mode": if include_details { "full" } else { "summary" },
            "max_items": max_items,
            "topologies": {
                "local": topology(&local),
                "team": topology(&team),
                "promise_parity": {
                    "compared": parity.compared,
                    "holds": parity.holds(),
                    "differences": parity.differences,
                },
                "technology_is_not_promise_parity": true,
            },
            "data_classes": DataClass::ALL.into_iter().map(|class| json!({
                "class": class,
                "name": class.name(),
                "holds_immutable_evidence": class.holds_immutable_evidence(),
            })).collect::<Vec<_>>(),
            "deployment_planes": Plane::ALL.into_iter().map(|plane| json!({
                "plane": plane,
                "name": plane.name(),
                "control_plane": plane.is_control_plane(),
            })).collect::<Vec<_>>(),
            "tenant_patterns": tenant_patterns,
            "slo_objectives": declared_objective_names(),
            "service_contracts": {
                "summary": {
                    "satisfied": service_summary.satisfied,
                    "diverges": service_summary.diverges,
                    "not_implemented": service_summary.not_implemented,
                    "divergences": service_summary.divergences,
                    "total": service_summary.total(),
                },
                "entries": service_rows,
                "entry_count": service_entries.len(),
                "omitted_entries": service_entries.len().saturating_sub(max_items),
            },
            "metrics": {
                "metrics_schema_version": METRICS_SCHEMA_VERSION,
                "atlasx_schema_version": ATLASX_SCHEMA_VERSION,
                "named_in_scope": named_in_scope(),
                "named_but_undefined": NAMED_NEVER_DEFINED.len(),
                "defined_here": DEFINED_HERE,
                "undefined_metrics_returned": NAMED_NEVER_DEFINED.iter().take(max_items).collect::<Vec<_>>(),
                "omitted_undefined_metrics": NAMED_NEVER_DEFINED.len().saturating_sub(max_items),
                "undefined_is_not_zero": true,
            },
            "sdk": {
                "registration_note": conformance_note(),
                "execution_and_isolation_are_not_implied": true,
            },
            "limitations": [
                "reference topologies validate promise parity, not a live deployment or network route",
                "service audits compare the transcribed contracts with in-tree reports; they do not provision hosted services",
                "the metrics catalogue records undefined metric names and denominators where stated; it does not fabricate measurements",
            ],
        });
        if include_details {
            result["details"] = json!({
                "service_entries": service_entries,
                "undefined_metrics": NAMED_NEVER_DEFINED,
            });
        }
        Ok(result)
    }

    pub(super) fn ops_acceptance(&self, arguments: &Value) -> Result<Value, String> {
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if max_items == 0 || max_items > 1_000 {
            return Err("max_items must be between 1 and 1000".into());
        }
        let max_items = max_items as usize;
        let findings = bioprism_ops::alpha::report();
        let summary = bioprism_ops::alpha::summary();
        let returned = findings.iter().take(max_items).collect::<Vec<_>>();
        Ok(json!({
            "ok": true,
            "summary": {
                "met": summary.met,
                "refuted": summary.refuted,
                "unverifiable": summary.unverifiable,
                "total": summary.total(),
                "is_release_ready": summary.met == summary.total(),
                "is_decidable": summary.unverifiable == 0,
            },
            "findings": returned,
            "omitted_findings": findings.len().saturating_sub(max_items),
            "guarantees": [
                "acceptance criteria remain a closed typed enumeration with met, refuted, and unverifiable states",
                "unverifiable criteria are never counted as passes or folded into a percentage",
                "non-unverifiable findings retain the basis that can entail their verdict",
            ],
            "limitations": [
                "the library can inspect its embedded workspace manifest and linked types, not run a clean checkout, demo, CI workflow, or external service",
                "is_release_ready is a mechanical summary predicate, not a claim that the twelve unverifiable criteria passed",
                "signature verification remains refuted by the workspace's symmetric-only attestation boundary",
            ],
        }))
    }

    pub(super) fn ops_capacity(&self, arguments: &Value) -> Result<Value, String> {
        let model: CapacityModel = serde_json::from_value(
            arguments
                .get("model")
                .cloned()
                .ok_or("model is required and must be a serialized CapacityModel")?,
        )
        .map_err(|error| format!("invalid capacity model: {error}"))?;
        let workload: Workload = serde_json::from_value(
            arguments
                .get("workload")
                .cloned()
                .ok_or("workload is required and must be a serialized Workload")?,
        )
        .map_err(|error| format!("invalid workload: {error}"))?;
        if workload.operations().len() > 1_000 {
            return Err("workload may contain at most 1000 operations".into());
        }
        let projection = match model.project(&workload) {
            Ok(projection) => projection,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "stage": "projection",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "guarantee": "unbounded traversals and over-ceiling materialised artifacts are refused before a capacity number is produced",
                }));
            }
        };

        let demand = arguments
            .get("demand")
            .cloned()
            .map(serde_json::from_value::<Demand>)
            .transpose()
            .map_err(|error| format!("invalid demand: {error}"))?;
        if let Some(demand) = demand {
            if !demand.calls_per_epoch.is_finite() || demand.calls_per_epoch < 0.0 {
                return Err("demand.calls_per_epoch must be finite and non-negative".into());
            }
        }
        let plan = arguments
            .get("degradation_plan")
            .cloned()
            .map(serde_json::from_value::<DegradationPlan>)
            .transpose()
            .map_err(|error| format!("invalid degradation_plan: {error}"))?;
        let saturation = match (demand, plan) {
            (None, None) => Value::Null,
            (None, Some(_)) => return Err("degradation_plan requires demand".into()),
            (Some(_), None) => json!({
                "ok": false,
                "refusal": "demand requires a named degradation_plan before saturation is evaluated",
                "fail_closed": true,
            }),
            (Some(demand), Some(plan)) => json!({
                "ok": true,
                "value": projection.under(&demand, &plan),
            }),
        };
        Ok(json!({
            "ok": true,
            "workload": workload.name(),
            "memory_ceiling_bytes": model.memory_ceiling_bytes(),
            "projection": projection,
            "work_per_call": projection.work_per_call(),
            "utilisation": projection.utilisation(),
            "sustainable_calls_per_epoch": projection.sustainable_calls_per_epoch(),
            "peak_resident_bytes": projection.peak_resident_bytes(),
            "fully_measured": projection.is_fully_measured(),
            "assumptions": projection.assumptions(),
            "saturation": saturation,
            "guarantees": [
                "capacity numbers retain measured-versus-assumed inputs and never become bare unqualified rates",
                "unbounded work and over-ceiling materialisation fail before projection",
                "saturation carries an explicit visible degradation plan rather than silently relaxing correctness or policy",
            ],
            "limitations": [
                "the model is caller-supplied and does not benchmark, profile, schedule, or observe a live deployment",
                "work is measured in abstract units per epoch; no wall-clock latency, queueing distribution, or currency is inferred",
                "a saturation plan describes declared concessions; it does not enforce backpressure or provision capacity",
            ],
        }))
    }

    pub(super) fn research_ci_check(&self, arguments: &Value) -> Result<Value, String> {
        let raw = match (
            arguments.get("document").and_then(Value::as_str),
            arguments.get("result").cloned(),
        ) {
            (Some(_), Some(_)) => {
                return Err("provide either document or inline result, not both".into());
            }
            (Some(relative), None) => {
                let path = self.resolve(relative)?;
                if path.is_dir() {
                    return Err(
                        "research_ci_check requires a JSON result document, not a directory".into(),
                    );
                }
                self.read_json(&path)?
            }
            (None, Some(result)) => result,
            (None, None) => {
                return Err("research_ci_check requires document or inline result".into());
            }
        };
        let result: ResultUnderReview = serde_json::from_value(raw)
            .map_err(|error| format!("invalid research result document: {error}"))?;
        let report = CiReport::full(result.subject.clone(), &result);
        let publishability = report.publishability();
        let publishable = matches!(
            publishability,
            bioprism_atlashub::Publishability::Publishable
        );
        let checks = report
            .outcomes
            .iter()
            .map(|(check, outcome)| {
                json!({
                    "check": check.as_str(),
                    "outcome": outcome,
                    "passed": outcome.is_pass(),
                })
            })
            .collect::<Vec<_>>();
        let failed = report
            .outcomes
            .iter()
            .filter(|(_, outcome)| matches!(outcome, bioprism_atlashub::CheckOutcome::Fail { .. }))
            .map(|(check, _)| check.as_str())
            .collect::<Vec<_>>();
        let undetermined = report
            .outcomes
            .iter()
            .filter(|(_, outcome)| {
                matches!(
                    outcome,
                    bioprism_atlashub::CheckOutcome::Undetermined { .. }
                )
            })
            .map(|(check, _)| check.as_str())
            .collect::<Vec<_>>();

        Ok(json!({
            "ok": true,
            "subject": result.subject,
            "observation_count": result.observations.len(),
            "check_count": checks.len(),
            "publishable": publishable,
            "publishability": publishability,
            "failed_checks": failed,
            "undetermined_checks": undetermined,
            "checks": checks,
            "lines": report.lines(),
            "limitations": [
                "this runs deterministic predicates over observations supplied by the caller",
                "it does not inspect a repository, execute a workflow, recompute a figure, or authenticate an external CI runner",
                "undetermined is blocked publication, not a passing result",
            ],
        }))
    }

    pub(super) fn release_audit(&self, arguments: &Value) -> Result<Value, String> {
        let raw_checks = arguments
            .get("checks")
            .and_then(Value::as_array)
            .ok_or("checks is required and must be an array of release check requests")?;
        if raw_checks.is_empty() || raw_checks.len() > 32 {
            return Err("checks must contain between 1 and 32 release check requests".into());
        }
        let include_details = arguments
            .get("include_details")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let encoded = serde_json::to_vec(&json!({
            "checks": raw_checks,
            "include_details": include_details,
        }))
        .map_err(|error| format!("cannot measure release-audit input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("release-audit input exceeds the 20000000-byte safety bound".into());
        }

        let mut rows = Vec::with_capacity(raw_checks.len());
        let mut blockers = Vec::new();
        let mut invocation_failures = 0usize;
        let mut required_count = 0usize;

        for (index, raw_check) in raw_checks.iter().enumerate() {
            let kind = raw_check
                .get("kind")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("checks[{index}].kind is required"))?;
            let arguments = raw_check
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            if !arguments.is_object() {
                return Err(format!("checks[{index}].arguments must be an object"));
            }
            let advisory_kind = matches!(kind, "repository_impact" | "developer_platform_status");
            let required = raw_check
                .get("required")
                .and_then(Value::as_bool)
                .unwrap_or(!advisory_kind);
            if advisory_kind && required {
                return Err(format!(
                    "{kind} is advisory-only and cannot be marked required"
                ));
            }
            if required {
                required_count += 1;
            }

            let result = match kind {
                "registry_gate" => self.registry_gate(&arguments),
                "bundle_verify" => self.bundle_verify(&arguments),
                "conformance_run" => self.conformance_run(&arguments),
                "research_ci_check" => self.research_ci_check(&arguments),
                "quality_gate_run" => self.quality_gate_run(&arguments),
                "ops_acceptance" => self.ops_acceptance(&arguments),
                "pack_health_assess" => self.pack_health_assess(&arguments),
                "repository_impact" => self.repository_impact(&arguments),
                "developer_platform_status" => self.developer_platform_status(&arguments),
                other => Err(format!(
                    "unknown release check {other:?}; choose registry_gate, bundle_verify, conformance_run, research_ci_check, quality_gate_run, ops_acceptance, pack_health_assess, repository_impact, or developer_platform_status"
                )),
            };

            match result {
                Ok(result) => {
                    let gate = release_gate_outcome(kind, &result);
                    let passed = gate == Some(true);
                    if required && !passed {
                        blockers.push(json!({
                            "index": index,
                            "kind": kind,
                            "reason": result
                                .get("refusal")
                                .and_then(Value::as_str)
                                .unwrap_or("required release check did not pass"),
                            "fail_closed": result
                                .get("fail_closed")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                        }));
                    }
                    let digest = bioprism_ids::ContentHash::of_value(&result)
                        .map_err(|error| format!("cannot digest {kind} result: {error}"))?;
                    let mut row = json!({
                        "index": index,
                        "kind": kind,
                        "required": required,
                        "advisory": !required,
                        "evaluated": true,
                        "gate": gate,
                        "passed": passed,
                        "result_digest": digest,
                        "result_ok": result.get("ok").and_then(Value::as_bool),
                        "refusal": result.get("refusal"),
                        "fail_closed": result.get("fail_closed"),
                    });
                    if include_details {
                        row["result"] = result;
                    }
                    rows.push(row);
                }
                Err(refusal) => {
                    invocation_failures += 1;
                    if required {
                        blockers.push(json!({
                            "index": index,
                            "kind": kind,
                            "reason": refusal,
                            "fail_closed": true,
                        }));
                    }
                    rows.push(json!({
                        "index": index,
                        "kind": kind,
                        "required": required,
                        "advisory": !required,
                        "evaluated": false,
                        "gate": Value::Null,
                        "passed": false,
                        "refusal": refusal,
                        "fail_closed": true,
                    }));
                }
            }
        }

        let release_ready = invocation_failures == 0
            && required_count > 0
            && blockers.is_empty()
            && rows
                .iter()
                .filter(|row| row["required"] == json!(true))
                .all(|row| row["passed"] == json!(true));
        Ok(json!({
            "ok": true,
            "release_ready": release_ready,
            "required_check_count": required_count,
            "check_count": rows.len(),
            "invocation_failures": invocation_failures,
            "blocking_count": blockers.len(),
            "blockers": blockers,
            "checks": rows,
            "guarantees": [
                "release readiness is the conjunction of named required gates; no score or advisory can offset a blocker",
                "verification refusals, invocation failures, and advisory observations remain distinct in the audit projection",
                "bundle, registry, quality, conformance, research-CI, operations, and pack-health checks delegate to their typed implementations",
                "repository impact and developer-platform status are visible advisory evidence and cannot be promoted into release gates by accident",
                "the projection is bounded, deterministic, local, and side-effect free; it does not publish, sign, deploy, or mutate artifacts",
            ],
            "limitations": [
                "a passing composition is evidence that supplied local checks passed, not a cryptographic signature, security scan, scientific validity claim, or deployment approval",
                "referenced documents and paths retain the limitations of the delegated tool and are not fetched from a network",
            ],
        }))
    }
}

fn release_gate_outcome(kind: &str, result: &Value) -> Option<bool> {
    match kind {
        "registry_gate" => result.get("passed").and_then(Value::as_bool),
        "bundle_verify" => result.get("ok").and_then(Value::as_bool),
        "conformance_run" => Some(
            result
                .get("release_decision")
                .and_then(|decision| decision.get("decision"))
                .and_then(Value::as_str)
                == Some("release"),
        ),
        "research_ci_check" => result.get("publishable").and_then(Value::as_bool),
        "quality_gate_run" => result.get("passed").and_then(Value::as_bool),
        "ops_acceptance" => result
            .get("summary")
            .and_then(|summary| summary.get("is_release_ready"))
            .and_then(Value::as_bool),
        "pack_health_assess" => result
            .get("score_gate")
            .and_then(|gate| gate.get("reportable"))
            .and_then(Value::as_bool),
        "repository_impact" | "developer_platform_status" => None,
        _ => None,
    }
}
