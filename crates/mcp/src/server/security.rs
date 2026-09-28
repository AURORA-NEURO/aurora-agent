//! MCP Security and privacy assurance handlers.
//!
//! Kept beside the dispatcher so each domain surface can be reviewed independently.

use super::*;

impl Server {
    pub(super) fn security_privacy_audit(&self, arguments: &Value) -> Result<Value, String> {
        let raw_manifest = arguments
            .get("manifest")
            .cloned()
            .ok_or("manifest is required and must be a serialized SecurityPrivacyManifest")?;
        let encoded = serde_json::to_vec(&raw_manifest)
            .map_err(|error| format!("cannot measure security/privacy manifest: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("manifest exceeds the 20000000-byte safety bound".into());
        }
        let manifest: SecurityPrivacyManifest = serde_json::from_value(raw_manifest)
            .map_err(|error| format!("invalid security/privacy manifest: {error}"))?;
        let audit = manifest
            .audit()
            .map_err(|error| format!("cannot audit security/privacy manifest: {error}"))?;
        let blocking_issue_count = audit
            .issues
            .iter()
            .filter(|issue| {
                issue.severity == bioprism_devplat::SecurityPrivacyIssueSeverity::Blocking
            })
            .count();
        let warning_count = audit
            .issues
            .iter()
            .filter(|issue| {
                issue.severity == bioprism_devplat::SecurityPrivacyIssueSeverity::Warning
            })
            .count();
        Ok(json!({
            "ok": true,
            "workflow": "security_privacy_audit",
            "schema": SECURITY_PRIVACY_AUDIT_SCHEMA,
            "manifest_digest": audit.digest,
            "valid": audit.valid,
            "security_privacy_ready": audit.valid,
            "blocking_issue_count": blocking_issue_count,
            "warning_count": warning_count,
            "audit": audit,
            "guarantees": [
                "asset classification, purpose, retention, residency, and deletion remain explicit",
                "permitted flows, identity hardening, threat treatment, independent reviews, and controls are audited as separate layers",
                "high and critical untreated threats cannot be converted into a clean posture by a caller-supplied boolean",
            ],
            "limitations": [
                "the route does not scan infrastructure, authenticate identities, verify a legal basis, or erase data",
                "the route does not execute red-team actions, test encryption, contact vendors, or mutate access systems",
                "evidence digests, classifications, review status, control state, and authorization records are caller-declared",
            ],
        }))
    }

    pub(super) fn security_program_audit(&self, arguments: &Value) -> Result<Value, String> {
        let raw_manifest = arguments
            .get("manifest")
            .cloned()
            .ok_or("manifest is required and must be a serialized SecurityProgramManifest")?;
        let encoded = serde_json::to_vec(&raw_manifest)
            .map_err(|error| format!("cannot measure security program manifest: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("manifest exceeds the 20000000-byte safety bound".into());
        }
        let manifest: SecurityProgramManifest = serde_json::from_value(raw_manifest)
            .map_err(|error| format!("invalid security program manifest: {error}"))?;
        let audit = manifest
            .audit()
            .map_err(|error| format!("cannot audit security program manifest: {error}"))?;
        let blocking_issue_count = audit
            .issues
            .iter()
            .filter(|issue| {
                issue.severity == bioprism_devplat::SecurityProgramIssueSeverity::Blocking
            })
            .count();
        let warning_count = audit
            .issues
            .iter()
            .filter(|issue| {
                issue.severity == bioprism_devplat::SecurityProgramIssueSeverity::Warning
            })
            .count();
        Ok(json!({
            "ok": true,
            "workflow": "security_program_audit",
            "schema": SECURITY_PROGRAM_AUDIT_SCHEMA,
            "manifest_digest": audit.digest,
            "valid": audit.valid,
            "security_program_ready": audit.valid,
            "blocking_issue_count": blocking_issue_count,
            "warning_count": warning_count,
            "audit": audit,
            "guarantees": [
                "authorized scope, campaign independence, evidence, findings, remediation, incidents, disclosure, and regression controls remain separate evidence layers",
                "high and critical findings cannot become ready without evidence, action, incident linkage, and bounded closure",
                "disclosure sequencing and public-safety review are explicit rather than hidden in a delivery boolean",
            ],
            "limitations": [
                "the route does not run scanners, fuzzers, probes, sandboxes, containment actions, or live controls",
                "the route does not contact vendors, publish disclosures, mutate incidents, or verify a production boundary",
                "scope authorization, evidence digests, approvals, timestamps, and control states are caller-declared",
            ],
        }))
    }

    /// Replay the executable parts of the section-13 red-team and incident-response contracts.
    ///
    /// This endpoint intentionally combines adjacent safety workflows because a useful security
    /// review follows a finding from discovery through regression protection, disclosure, boundary
    /// analysis, incident containment, and evidence. Each sub-workflow is still independently
    /// typed and fail-closed: a malformed row does not become a green result, and a refusal is
    /// returned beside the other rows rather than being erased by a best-effort summary.
    pub(super) fn security_redteam_simulate(&self, arguments: &Value) -> Result<Value, String> {
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if !(1..=1_000).contains(&max_items) {
            return Err("max_items must be between 1 and 1000".into());
        }
        let max_items = max_items as usize;
        let include_details = arguments
            .get("include_details")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let finding_inputs = input_array(arguments, "findings", 256)?;
        let vulnerability_inputs = input_array(arguments, "vulnerabilities", 256)?;
        let delivery_inputs = input_array(arguments, "deliveries", 256)?;
        let incident_inputs = input_array(arguments, "incidents", 256)?;
        let audit_inputs = input_array(arguments, "audit_records", 512)?;
        let attestation_inputs = input_array(arguments, "attestations", 256)?;

        let mut corpus = DisclosureCorpus::default();
        let mut finding_rows = Vec::with_capacity(finding_inputs.len());
        for (index, raw) in finding_inputs.iter().enumerate() {
            let row = match raw.as_object() {
                Some(object) => {
                    let parsed = (|| -> Result<Value, String> {
                        let id = required_string(object, "id")?;
                        let campaign = required_string(object, "campaign")?;
                        let boundary = required_string(object, "boundary")?;
                        let class: VulnerabilityClass = parse_field(object, "class")?;
                        let status: FindingStatus = object
                            .get("status")
                            .cloned()
                            .map(|value| parse_value(value, "status"))
                            .transpose()?
                            .unwrap_or(FindingStatus::Reported);
                        let finding =
                            Finding::new(id, campaign, boundary, class).with_status(status);
                        let finding = object
                            .get("reproduction")
                            .and_then(Value::as_str)
                            .map(|reproduction| finding.clone().reproducing(reproduction))
                            .unwrap_or(finding);
                        let finding_view = serde_json::to_value(&finding)
                            .map_err(|error| format!("finding serialization failed: {error}"))?;
                        let embargoed = object
                            .get("embargoed")
                            .and_then(Value::as_bool)
                            .unwrap_or(true);
                        let minimise = object
                            .get("minimised")
                            .and_then(Value::as_bool)
                            .unwrap_or(false);
                        let regression = match finding.clone().into_regression_cell(embargoed) {
                            Ok(cell) => {
                                let cell = if minimise { cell.minimised() } else { cell };
                                corpus.push(cell.clone());
                                json!({
                                    "eligible": true,
                                    "cell": cell,
                                    "public_summary": cell.public_summary(),
                                })
                            }
                            Err(error) => json!({
                                "eligible": false,
                                "refusal": error.to_string(),
                                "fail_closed": true,
                            }),
                        };
                        Ok(json!({
                            "index": index,
                            "ok": true,
                            "finding": finding_view,
                            "regression_gate": regression,
                        }))
                    })();
                    parsed.unwrap_or_else(|error| {
                        json!({
                            "index": index,
                            "ok": false,
                            "refusal": error,
                            "fail_closed": true,
                        })
                    })
                }
                None => json!({
                    "index": index,
                    "ok": false,
                    "refusal": "each finding must be an object",
                    "fail_closed": true,
                }),
            };
            finding_rows.push(row);
        }

        let boundary_universe = arguments
            .get("boundary_universe")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let universe_refs = boundary_universe
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let corpus_rows = json!({
            "sentinel_count": corpus.sentinel_count(),
            "covered_boundaries": corpus.covered_boundaries(),
            "unminimised_count": corpus.unminimised().len(),
            "uncovered_boundaries": corpus.uncovered(&universe_refs),
            "cells": if include_details {
                json!(corpus.cells.iter().take(max_items).collect::<Vec<_>>())
            } else {
                json!([])
            },
            "omitted_cells": corpus.cells.len().saturating_sub(max_items),
        });

        let mut vulnerability_rows = Vec::with_capacity(vulnerability_inputs.len());
        for (index, raw) in vulnerability_inputs.iter().enumerate() {
            let row = match raw.as_object() {
                Some(object) => {
                    let parsed = (|| -> Result<Value, String> {
                        let id = required_string(object, "id")?;
                        let class: VulnerabilityClass = parse_field(object, "class")?;
                        let severity: SafetySeverity = parse_field(object, "severity")?;
                        let epoch = required_u64(object, "epoch")?;
                        let impact: ImpactAxes = object
                            .get("impact")
                            .cloned()
                            .map(parse_impact_axes)
                            .transpose()?
                            .unwrap_or_default();
                        let mut vulnerability =
                            Vulnerability::reported(id, class, severity, epoch).impacting(impact);
                        let advisory: Option<Advisory> = object
                            .get("advisory")
                            .cloned()
                            .map(|value| parse_value(value, "advisory"))
                            .transpose()?;
                        let transition_inputs = object_array(object, "transitions", 64)?;
                        let mut transition_rows = Vec::with_capacity(transition_inputs.len());
                        let mut stopped = false;
                        for (transition_index, transition) in transition_inputs.iter().enumerate() {
                            if stopped {
                                break;
                            }
                            let Some(transition) = transition.as_object() else {
                                transition_rows.push(json!({
                                    "index": transition_index,
                                    "ok": false,
                                    "refusal": "each transition must be an object",
                                    "fail_closed": true,
                                }));
                                stopped = true;
                                continue;
                            };
                            let attempted = (|| -> Result<Value, String> {
                                let to: Stage = parse_field(transition, "to")?;
                                let epoch = required_u64(transition, "epoch")?;
                                let note =
                                    transition.get("note").and_then(Value::as_str).unwrap_or("");
                                if to == Stage::Disclosed {
                                    let gate = advisory.clone().unwrap_or_default();
                                    gate.audit_for(&vulnerability)
                                        .map_err(|error| error.to_string())?;
                                }
                                vulnerability
                                    .advance(DisclosureTransition::to(to, epoch).noting(note))
                                    .map_err(|error| error.to_string())?;
                                Ok(json!({
                                    "index": transition_index,
                                    "ok": true,
                                    "to": to,
                                    "epoch": epoch,
                                    "stage_after": vulnerability.stage,
                                }))
                            })();
                            match attempted {
                                Ok(value) => transition_rows.push(value),
                                Err(error) => {
                                    transition_rows.push(json!({
                                        "index": transition_index,
                                        "ok": false,
                                        "refusal": error,
                                        "fail_closed": true,
                                        "stage_after": vulnerability.stage,
                                    }));
                                    stopped = true;
                                }
                            }
                        }
                        let missing_advisory_fields = advisory
                            .as_ref()
                            .map(Advisory::missing_fields)
                            .unwrap_or_else(|| Advisory::default().missing_fields());
                        Ok(json!({
                            "index": index,
                            "ok": true,
                            "vulnerability": vulnerability,
                            "transitions": transition_rows,
                            "transition_count": transition_inputs.len(),
                            "stopped_after_refusal": stopped,
                            "advisory_present": advisory.is_some(),
                            "advisory_missing_fields": missing_advisory_fields,
                            "independent_verification_required": severity.requires_independent_verification(),
                            "disclosed": vulnerability.stage == Stage::Disclosed,
                        }))
                    })();
                    parsed.unwrap_or_else(|error| {
                        json!({
                            "index": index,
                            "ok": false,
                            "refusal": error,
                            "fail_closed": true,
                        })
                    })
                }
                None => json!({
                    "index": index,
                    "ok": false,
                    "refusal": "each vulnerability must be an object",
                    "fail_closed": true,
                }),
            };
            vulnerability_rows.push(row);
        }

        let boundary_model = BoundaryModel::evaluation_model();
        let within_trial_agent_to_evaluator = boundary_model
            .influence_paths_within_trial(TrustZone::AgentSandbox, TrustZone::EvaluatorSandbox);
        let within_trial_evaluator_to_agent = boundary_model
            .influence_paths_within_trial(TrustZone::EvaluatorSandbox, TrustZone::AgentSandbox);
        let all_scope_agent_to_evaluator =
            boundary_model.influence_paths(TrustZone::AgentSandbox, TrustZone::EvaluatorSandbox);
        let feedback_loops = boundary_model.feedback_loops();
        let mut delivery_rows = Vec::with_capacity(delivery_inputs.len());
        for (index, raw) in delivery_inputs.iter().enumerate() {
            let row = match raw.as_object() {
                Some(object) => {
                    let parsed = (|| -> Result<Value, String> {
                        let id = required_string(object, "id")?;
                        let kind: ArtifactKind = parse_field(object, "kind")?;
                        let origin: TrustZone = parse_field(object, "origin")?;
                        let to: TrustZone = parse_field(object, "to")?;
                        let via: Channel = parse_field(object, "via")?;
                        let artifact = MovingArtifact::new(id, kind, origin);
                        match boundary_model.deliver(&artifact, to, via) {
                            Ok(crossing) => Ok(json!({
                                "index": index,
                                "ok": true,
                                "crossing": crossing,
                                "honest_label": crossing.honest_label(),
                                "scope": boundary_model.scope_of(origin, to, via),
                            })),
                            Err(error) => Ok(json!({
                                "index": index,
                                "ok": false,
                                "refusal": error.to_string(),
                                "fail_closed": true,
                                "requested": {
                                    "artifact": artifact,
                                    "to": to,
                                    "via": via,
                                },
                            })),
                        }
                    })();
                    parsed.unwrap_or_else(|error| {
                        json!({
                            "index": index,
                            "ok": false,
                            "refusal": error,
                            "fail_closed": true,
                        })
                    })
                }
                None => json!({
                    "index": index,
                    "ok": false,
                    "refusal": "each delivery must be an object",
                    "fail_closed": true,
                }),
            };
            delivery_rows.push(row);
        }
        let delivery_allowed = delivery_rows
            .iter()
            .filter(|row| row.get("ok") == Some(&json!(true)))
            .count();
        let delivery_refused = delivery_rows.len().saturating_sub(delivery_allowed);

        let mut incident_rows = Vec::with_capacity(incident_inputs.len());
        for (index, raw) in incident_inputs.iter().enumerate() {
            let row = match raw.as_object() {
                Some(object) => {
                    let parsed = (|| -> Result<Value, String> {
                        let id = required_string(object, "id")?;
                        let class: IncidentClass = parse_field(object, "class")?;
                        let opened_at = required_u64(object, "opened_at")?;
                        let mut incident = Incident::open(id, class, opened_at);
                        let request_inputs = object_array(object, "requests", 64)?;
                        let mut request_rows = Vec::with_capacity(request_inputs.len());
                        for (request_index, raw_request) in request_inputs.iter().enumerate() {
                            let parsed_request = (|| -> Result<Value, String> {
                                let request = raw_request
                                    .as_object()
                                    .ok_or("each containment request must be an object")?;
                                let action: ContainmentAction = parse_field(request, "action")?;
                                let requested_by = required_string(request, "requested_by")?;
                                let requested_at = required_u64(request, "requested_at")?;
                                let request =
                                    ContainmentRequest::new(action, requested_by, requested_at);
                                let label = request.honest_label();
                                incident.requests.push(request.clone());
                                Ok(json!({
                                    "index": request_index,
                                    "ok": true,
                                    "request": request,
                                    "honest_label": label,
                                }))
                            })();
                            request_rows.push(parsed_request.unwrap_or_else(|error| {
                                json!({
                                    "index": request_index,
                                    "ok": false,
                                    "refusal": error,
                                    "fail_closed": true,
                                })
                            }));
                        }

                        if let Some(radius) = object.get("blast_radius") {
                            incident = incident.with_blast_radius(parse_blast_radius(radius)?);
                        }
                        let timeline_inputs = object_array(object, "timeline", 256)?;
                        let mut timeline_rows = Vec::with_capacity(timeline_inputs.len());
                        for (timeline_index, raw_entry) in timeline_inputs.iter().enumerate() {
                            let result = (|| -> Result<Value, String> {
                                let entry = raw_entry
                                    .as_object()
                                    .ok_or("each timeline entry must be an object")?;
                                let epoch = required_u64(entry, "epoch")?;
                                let actor = required_string(entry, "actor")?;
                                let event = required_string(entry, "event")?;
                                incident
                                    .timeline
                                    .push(epoch, actor, event)
                                    .map_err(|error| error.to_string())?;
                                Ok(json!({
                                    "index": timeline_index,
                                    "ok": true,
                                    "epoch": epoch,
                                }))
                            })();
                            timeline_rows.push(result.unwrap_or_else(|error| {
                                json!({
                                    "index": timeline_index,
                                    "ok": false,
                                    "refusal": error,
                                    "fail_closed": true,
                                })
                            }));
                        }

                        let expected = if let Some(raw_expected) = object.get("expected_actions") {
                            input_enum_array::<ContainmentAction>(
                                raw_expected,
                                "expected_actions",
                                32,
                            )?
                        } else {
                            vec![
                                ContainmentAction::StopExecutionPool,
                                ContainmentAction::RevokeLeases,
                                ContainmentAction::RevokeCredentials,
                                ContainmentAction::QuarantineArtifacts,
                                ContainmentAction::FreezePublication,
                                ContainmentAction::PreserveLogs,
                                ContainmentAction::RotateKeys,
                                ContainmentAction::NotifyFederationPeers,
                            ]
                        };
                        let report = incident.report_contained();
                        let unrequested = incident.unrequested_actions(&expected);
                        let report_value = match report {
                            Ok(report) => json!({
                                "allowed": true,
                                "report": report,
                                "caveat": "requested actions are not observed executions",
                            }),
                            Err(error) => json!({
                                "allowed": false,
                                "refusal": error.to_string(),
                                "fail_closed": true,
                            }),
                        };
                        Ok(json!({
                            "index": index,
                            "ok": true,
                            "incident": incident,
                            "requests": request_rows,
                            "timeline": timeline_rows,
                            "containment_claim": report_value,
                            "unrequested_actions": unrequested,
                            "result_tainting_class": class.taints_results(),
                        }))
                    })();
                    parsed.unwrap_or_else(|error| {
                        json!({
                            "index": index,
                            "ok": false,
                            "refusal": error,
                            "fail_closed": true,
                        })
                    })
                }
                None => json!({
                    "index": index,
                    "ok": false,
                    "refusal": "each incident must be an object",
                    "fail_closed": true,
                }),
            };
            incident_rows.push(row);
        }

        let mut audit_log = AuditLog::new();
        let mut audit_rows = Vec::with_capacity(audit_inputs.len());
        for (index, raw) in audit_inputs.iter().enumerate() {
            let result = (|| -> Result<Value, String> {
                let object = raw
                    .as_object()
                    .ok_or("each audit record must be an object")?;
                let event: AuditEvent = parse_field(object, "event")?;
                let actor = required_string(object, "actor")?;
                let subject = required_string(object, "subject")?;
                let epoch = required_u64(object, "epoch")?;
                let statement = parse_safety_statement(
                    object
                        .get("statement")
                        .ok_or("audit record requires statement")?,
                )?;
                let record = AuditRecord::new(event, actor, subject, statement, epoch);
                match audit_log.append(record) {
                    Ok(linked) => Ok(json!({
                        "index": index,
                        "ok": true,
                        "linked": linked,
                    })),
                    Err(error) => Ok(json!({
                        "index": index,
                        "ok": false,
                        "refusal": error.to_string(),
                        "fail_closed": true,
                    })),
                }
            })();
            audit_rows.push(result.unwrap_or_else(|error| {
                json!({
                    "index": index,
                    "ok": false,
                    "refusal": error,
                    "fail_closed": true,
                })
            }));
        }
        let audit_verification = audit_log.verify();

        let mut attestation_rows = Vec::with_capacity(attestation_inputs.len());
        for (index, raw) in attestation_inputs.iter().enumerate() {
            let result = (|| -> Result<Value, String> {
                let object = raw
                    .as_object()
                    .ok_or("each attestation must be an object")?;
                let claim = parse_attestation_claim(object)?;
                let observed = object
                    .get("observed")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let by = object.get("by").and_then(Value::as_str).unwrap_or("caller");
                let attestation = if observed {
                    let component = object
                        .get("component")
                        .and_then(Value::as_str)
                        .unwrap_or("unspecified")
                        .to_string();
                    Attestation::observed(
                        claim,
                        SafetyObservation::DigestsCompared {
                            component,
                            equal: object.get("equal").and_then(Value::as_bool).unwrap_or(true),
                        },
                    )
                    .map_err(|error| error.to_string())?
                } else {
                    Attestation::asserted(claim, by)
                };
                Ok(json!({
                    "index": index,
                    "ok": true,
                    "observed": attestation.is_observed(),
                    "attestation": attestation,
                }))
            })();
            attestation_rows.push(result.unwrap_or_else(|error| {
                json!({
                    "index": index,
                    "ok": false,
                    "refusal": error,
                    "fail_closed": true,
                })
            }));
        }

        let bounded = |rows: &[Value]| rows.iter().take(max_items).cloned().collect::<Vec<_>>();
        Ok(json!({
            "ok": true,
            "workflow": "section_13_redteam_incident_evidence",
            "input_counts": {
                "findings": finding_inputs.len(),
                "vulnerabilities": vulnerability_inputs.len(),
                "deliveries": delivery_inputs.len(),
                "incidents": incident_inputs.len(),
                "audit_records": audit_inputs.len(),
                "attestations": attestation_inputs.len(),
            },
            "findings": bounded(&finding_rows),
            "findings_omitted": finding_rows.len().saturating_sub(max_items),
            "regression_corpus": corpus_rows,
            "vulnerabilities": bounded(&vulnerability_rows),
            "vulnerabilities_omitted": vulnerability_rows.len().saturating_sub(max_items),
            "boundary": {
                "model": "evaluation_model",
                "within_trial_agent_to_evaluator": within_trial_agent_to_evaluator,
                "within_trial_evaluator_to_agent": within_trial_evaluator_to_agent,
                "all_scope_agent_to_evaluator": all_scope_agent_to_evaluator,
                "feedback_loops": feedback_loops,
                "delivery_rows": bounded(&delivery_rows),
                "delivery_rows_omitted": delivery_rows.len().saturating_sub(max_items),
                "allowed_delivery_count": delivery_allowed,
                "refused_delivery_count": delivery_refused,
            },
            "incidents": bounded(&incident_rows),
            "incidents_omitted": incident_rows.len().saturating_sub(max_items),
            "audit": {
                "rows": bounded(&audit_rows),
                "rows_omitted": audit_rows.len().saturating_sub(max_items),
                "chain_length": audit_log.len(),
                "head": audit_log.head(),
                "verified": audit_verification.is_ok(),
                "verification_refusal": audit_verification.err().map(|error| error.to_string()),
                "assertion_count": audit_log.assertions().len(),
                "public_view_count": audit_log.public_view().len(),
                "records": if include_details {
                    json!(audit_log.records().iter().take(max_items).collect::<Vec<_>>())
                } else {
                    json!([])
                },
            },
            "attestations": bounded(&attestation_rows),
            "attestations_omitted": attestation_rows.len().saturating_sub(max_items),
            "guarantees": [
                "only confirmed findings can become regression cells",
                "embargo hides reproduction detail but never hides sentinel existence",
                "vulnerability disclosure advances one lifecycle rung at a time and requires a complete advisory",
                "unmodelled or forbidden artifact edges fail closed",
                "within-trial evaluator-to-agent influence is absent while across-trial feedback is reported explicitly",
                "partial or unresolved blast radius cannot produce a containment report",
                "audit records distinguish observations from assertions and verify a hash-linked chain",
                "unwitnessable attestations remain assertions rather than being minted as observations",
            ],
            "limitations": [
                "this endpoint replays typed contracts; it does not run fuzzers, detectors, sandboxes, processes, containers, network controls, credential revocation, quarantine, or publication freezes",
                "epochs, findings, lineage, and requested containment actions are caller-supplied inputs",
                "the audit chain is in-memory and has no external checkpoint, signature, independent witness, or durable storage",
                "a permitted boundary crossing is evidence that the model allowed an intent, not evidence that a transfer was observed",
                "there is no intake queue, notification service, bounty/CVE workflow, triage operator, or incident commander",
            ],
        }))
    }

    pub(super) fn safety_posture(&self, arguments: &Value) -> Result<Value, String> {
        let include_threats = arguments
            .get("include_threats")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let model = section_13();
        let coverage = model.coverage();
        let residual = model
            .residual()
            .into_iter()
            .map(|threat| threat.id.clone())
            .collect::<Vec<_>>();
        let unanalysed = model
            .unanalysed()
            .into_iter()
            .map(|threat| threat.id.clone())
            .collect::<Vec<_>>();
        let unreachable = model
            .unreachable_threats()
            .into_iter()
            .map(|threat| threat.id.clone())
            .collect::<Vec<_>>();
        let mut result = json!({
            "ok": true,
            "model": "section_13",
            "adversaries": model.adversaries.len(),
            "threats": model.threats.len(),
            "coverage": coverage,
            "coverage_summary": coverage.summary(),
            "residual_threat_ids": residual,
            "unanalysed_threat_ids": unanalysed,
            "unreachable_threat_ids": unreachable,
            "audit_acceptances": model.audit_acceptances().is_ok(),
            "perimeter_controls_are_not_claimed_as_enforced": true,
        });
        if include_threats {
            result["threat_details"] = json!(model.threats);
        }
        Ok(result)
    }

    pub(super) fn safety_release_gate(&self, arguments: &Value) -> Result<Value, String> {
        let raw_assessment = arguments
            .get("assessment")
            .cloned()
            .ok_or("assessment is required and must contain subject, category, and ratings")?;
        let assessment: RiskAssessment = serde_json::from_value(raw_assessment)
            .map_err(|error| format!("invalid risk assessment: {error}"))?;
        let unrated = assessment
            .unrated()
            .into_iter()
            .map(|dimension| dimension.to_string())
            .collect::<Vec<_>>();
        let high_risk_dimensions = assessment
            .high_risk_dimensions()
            .into_iter()
            .map(|dimension| dimension.to_string())
            .collect::<Vec<_>>();
        let decision = ReleaseGate
            .decide(&assessment)
            .map_err(|error| format!("safety release gate refused: {error}"))?;
        let cleared = decision.is_cleared();
        Ok(json!({
            "ok": true,
            "subject": assessment.subject,
            "category": assessment.category,
            "decision": decision,
            "cleared": cleared,
            "unrated_dimensions": unrated,
            "high_risk_dimensions": high_risk_dimensions,
            "rule": "zero high non-mitigating dimensions clears; one conditions release; two or more block; any unrated dimension refuses the gate",
            "fail_closed": true,
            "limitations": [
                "ratings are reviewer-supplied labels; this process does not classify content or validate the reviewer",
                "the threshold is the explicit bioprism-safety rule because the blueprint does not specify numeric thresholds",
                "this is a release decision model, not runtime sandboxing, egress control, authentication, or clinical approval",
            ],
        }))
    }

    pub(super) fn medical_boundary_check(&self, arguments: &Value) -> Result<Value, String> {
        let raw_output = arguments
            .get("output")
            .cloned()
            .ok_or("output is required and must be a serialized RequestedOutput")?;
        let output: RequestedOutput = serde_json::from_value(raw_output)
            .map_err(|error| format!("invalid requested output: {error}"))?;
        let boundary = MedicalBoundary;
        match boundary.admit(&output) {
            Ok(use_case) => Ok(json!({
                "ok": true,
                "admitted": true,
                "use_case": use_case,
                "research_only_label": boundary.label(),
                "boundary_is_unconditional": true,
                "limitations": [
                    "admission means the requested category is inside the research-only model; it does not validate scientific quality or runtime safety",
                    "clinical outputs have no override, force flag, or reviewer bypass in this boundary",
                ],
            })),
            Err(error) => Ok(json!({
                "ok": false,
                "admitted": false,
                "refusal": error.to_string(),
                "research_only_label": boundary.label(),
                "boundary_is_unconditional": true,
                "clinical_output_is_never_admitted": true,
            })),
        }
    }

    pub(super) fn obligation_gate_check(&self, arguments: &Value) -> Result<Value, String> {
        let graph_value = arguments
            .get("graph")
            .cloned()
            .ok_or("graph is required and must be a serialized ObligationGraph")?;
        let graph_bytes = serde_json::to_vec(&graph_value)
            .map_err(|error| format!("cannot measure obligation graph envelope: {error}"))?;
        if graph_bytes.len() > 10_000_000 {
            return Err("obligation graph exceeds the 10000000-byte safety bound".into());
        }
        let graph: ObligationGraph = serde_json::from_value(graph_value)
            .map_err(|error| format!("invalid obligation graph: {error}"))?;
        if graph.len() > 10_000 {
            return Err("graph exceeds the 10000-obligation safety bound".into());
        }

        let action_value = arguments
            .get("action")
            .cloned()
            .ok_or("action is required and must be a serialized obligation Action")?;
        let action_bytes = serde_json::to_vec(&action_value)
            .map_err(|error| format!("cannot measure obligation action envelope: {error}"))?;
        if action_bytes.len() > 1_000_000 {
            return Err("obligation action exceeds the 1000000-byte safety bound".into());
        }
        let action: ObligationAction = serde_json::from_value(action_value)
            .map_err(|error| format!("invalid obligation action: {error}"))?;
        if action.prerequisites.len() > 1_000 {
            return Err("action exceeds the 1000-prerequisite safety bound".into());
        }

        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if max_items == 0 || max_items > 1_000 {
            return Err("max_items must be between 1 and 1000".into());
        }
        let max_items = max_items as usize;
        let gate = may_perform(&action, &graph);
        let validation = graph.validate().err().map(|error| error.to_string());
        let graph_hash = graph.content_hash().ok().map(|hash| hash.to_string());
        let topological = graph.topological_order().ok();
        let effective = graph.effective_states().ok();
        let frontier = graph.frontier().ok();
        let undischarged = graph.undischarged().ok();
        let topological_count = topological.as_ref().map_or(0, Vec::len);
        let effective_count = effective
            .as_ref()
            .map_or(0, std::collections::BTreeMap::len);
        let frontier_count = frontier.as_ref().map_or(0, Vec::len);
        let undischarged_count = undischarged.as_ref().map_or(0, Vec::len);
        let topological = topological
            .map(|ids| ids.into_iter().take(max_items).collect::<Vec<_>>())
            .unwrap_or_default();
        let effective = effective
            .map(|states| {
                states
                    .into_iter()
                    .take(max_items)
                    .map(|(obligation, state)| {
                        json!({
                            "obligation": obligation,
                            "effective": state,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let frontier = frontier
            .map(|entries| entries.into_iter().take(max_items).collect::<Vec<_>>())
            .unwrap_or_default();
        let undischarged = undischarged
            .map(|entries| {
                entries
                    .into_iter()
                    .take(max_items)
                    .map(|(obligation, state)| {
                        json!({
                            "obligation": obligation,
                            "effective": state,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let gate_value = serde_json::to_value(&gate)
            .map_err(|error| format!("cannot serialize obligation gate: {error}"))?;
        let refusal = if gate.is_allowed() {
            Value::Null
        } else {
            serde_json::to_value(gate.block_reason())
                .map_err(|error| format!("cannot serialize obligation refusal: {error}"))?
        };
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/obligation-gate-check/0.1",
            "outcome_kind": if gate.is_allowed() { "allowed" } else { "blocked" },
            "allowed": gate.is_allowed(),
            "goal": graph.goal.clone(),
            "action": action,
            "gate": gate_value,
            "refusal": refusal,
            "graph": {
                "valid": validation.is_none(),
                "validation_error": validation,
                "sha256": graph_hash,
                "obligation_count": graph.len(),
                "mandatory": graph.mandatory_ids(),
                "topological_order": topological,
                "omitted_topological": topological_count.saturating_sub(max_items),
                "effective_states": effective,
                "omitted_effective_states": effective_count.saturating_sub(max_items),
                "frontier": frontier,
                "omitted_frontier": frontier_count.saturating_sub(max_items),
                "undischarged": undischarged,
                "omitted_undischarged": undischarged_count.saturating_sub(max_items),
            },
            "guarantees": [
                "high-regret actions with undeclared prerequisites are blocked",
                "unknown obligations, dangling dependencies, and cycles cannot open a gate",
                "effective dependency states are computed by bioprism-obligation rather than by a boolean checklist",
                "irreversible actions answer to the full mandatory closure, including obligations they did not name",
                "frontier, effective states, and the gate decision remain separately inspectable",
            ],
            "limitations": [
                "the graph, state evidence, actor confidence, and action declarations are caller-supplied",
                "this tool does not authenticate authority, execute the action, or acquire missing evidence",
                "confidence is an actor assertion and is not calibrated into a probability",
                "truncated projection rows are bounded views, not complete graph claims",
            ],
        }))
    }

    pub(super) fn lens_leakage_check(&self, arguments: &Value) -> Result<Value, String> {
        let raw_cohort = arguments
            .get("cohort")
            .cloned()
            .ok_or("cohort is required and must be a serialized CohortSplit")?;
        let encoded = serde_json::to_vec(&raw_cohort)
            .map_err(|error| format!("cannot measure cohort envelope: {error}"))?;
        if encoded.len() > 5_000_000 {
            return Err("cohort exceeds the 5000000-byte safety bound".into());
        }
        let cohort: CohortSplit = serde_json::from_value(raw_cohort)
            .map_err(|error| format!("invalid lens cohort: {error}"))?;
        if cohort.subjects.is_empty() {
            return Err("cohort.subjects must contain at least one subject".into());
        }
        if cohort.subjects.len() > 10_000 {
            return Err("cohort.subjects may contain at most 10000 subjects".into());
        }
        if cohort.preprocessing.len() > 10_000 {
            return Err("cohort.preprocessing may contain at most 10000 steps".into());
        }
        for subject in &cohort.subjects {
            if subject.subject.is_empty() || subject.subject.len() > 256 {
                return Err("each subject id must contain between 1 and 256 bytes".into());
            }
            if subject.split.is_empty() || subject.split.len() > 256 {
                return Err("each split id must contain between 1 and 256 bytes".into());
            }
            if subject.aliases.len() > 256 {
                return Err("each subject may contain at most 256 aliases".into());
            }
        }
        for step in &cohort.preprocessing {
            if step.name.is_empty() || step.name.len() > 256 {
                return Err("each preprocessing name must contain between 1 and 256 bytes".into());
            }
            if step.fit_over.len() > 256 {
                return Err("each preprocessing step may name at most 256 splits".into());
            }
        }

        let raw_scope = arguments
            .get("scope")
            .cloned()
            .ok_or("scope is required; bind the lens's cohort dimension explicitly")?;
        let scope = ScopeKey::from_json(&raw_scope)
            .map_err(|error| format!("invalid lens scope: {error}"))?;
        let report = run_lens(&CohortLeakageLens, &scope, &cohort)
            .map_err(|error| format!("lens execution refused: {error}"))?;
        if report.witnesses().len() > 20_000 {
            return Err("lens report exceeds the 20000-witness safety bound".into());
        }
        let include_spoken = arguments
            .get("include_spoken")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let report_json = serde_json::to_value(&report)
            .map_err(|error| format!("cannot serialize sealed lens report: {error}"))?;

        Ok(json!({
            "ok": true,
            "lens": report.lens(),
            "blueprint_module": report.blueprint_module(),
            "scope": scope,
            "outcome": report.outcome().as_str(),
            "completeness": report.completeness(),
            "witness_count": report.witnesses().len(),
            "receipt": report.receipt(),
            "report": report_json,
            "spoken": include_spoken.then(|| report.spoken()),
            "guarantees": [
                "scope preconditions are checked before evidence is answered",
                "identity, site, temporal, and preprocessing findings remain distinct",
                "an unrunnable check is underdetermined rather than a clean pass",
                "the sealed report receipt covers the canonical nonvisual witness rows",
                "this lens detects leakage and never repairs or regenerates a split",
            ],
        }))
    }
}
