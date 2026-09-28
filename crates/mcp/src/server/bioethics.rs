//! MCP Bioethics and research-boundary handlers.
//!
//! Kept beside the dispatcher so each domain surface can be reviewed independently.

use super::*;

impl Server {
    pub(super) fn bioethics_evidence_surveillance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt =
            crate::research_contracts::operate_bioethics_evidence_surveillance_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": EVIDENCE_SURVEILLANCE_ASSURANCE_FEATURE_ID,
            "contract_version": EVIDENCE_SURVEILLANCE_ASSURANCE_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "multimodal evidence candidates remain partitioned into selected, unresolved, and blocked states",
                "dual-use, privacy, representation, institutional-authorization, bounded-autonomy, adversarial, policy, provenance, replay, and locality gates are deterministic and fail closed",
                "omissions, uncertainty, contradictions, negative evidence, and ethical gate failures remain observable",
                "qualified output is limited to a verification receipt; the tool never retrieves, exports raw data, or makes clinical decisions"
            ],
            "limitations": [
                "the harness verifies caller-supplied evidence declarations and review attestations; it does not classify biological content or independently authenticate reviewers",
                "a qualified receipt is a release-gate result, not a scientific conclusion, completed retrieval, or clinical advice"
            ]
        }))
    }

    pub(super) fn bioethics_prospective_computational_execution_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a ResearchWorkflowSpec4")?;
        let receipt =
            crate::research_contracts::operate_bioethics_prospective_computational_execution_json(
                request,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_bioethics::PROSPECTIVE_COMPUTATIONAL_EXECUTION_FEATURE_ID,
            "contract_version": bioprism_bioethics::PROSPECTIVE_COMPUTATIONAL_EXECUTION_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "A1 prospective high-throughput computational plans are deterministically ordered and verified without dispatching code",
                "cycles, missing dependencies, budget exhaustion, policy failures, non-local effects, replay mismatches, unknown evidence, omissions, contradictions, and negative results remain explicit and fail closed",
                "qualified output is a content-addressed execution-plan receipt with aggregate-digest-only federation and no raw-data movement"
            ],
            "limitations": [
                "the harness evaluates caller-supplied workflow declarations and does not start processes, contact providers, or inspect raw data",
                "a qualified run is release evidence for bounded preclinical computation, not proof of scientific validity or workflow completion; no clinical decisions are made"
            ]
        }))
    }

    pub(super) fn bioethics_scale_frontier_contract(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt = crate::research_contracts::operate_bioethics_scale_frontier_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_bioethics::BIOETHICS_CAPACITY_FEATURE_ID,
            "contract_version": bioprism_bioethics::BIOETHICS_CAPACITY_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "capacity workloads are deterministically ordered and partitioned into selected, unresolved, blocked, and missing states",
                "dual-use, privacy, institutional authorization, no-clinical-use, policy, protected-closure, signed-approval, federation, replay, provenance, locality, budget, and adversarial gates fail closed",
                "scale evidence and ethical controls remain explicit; the route never schedules work, grants authority, or treats throughput as safety proof"
            ],
            "limitations": [
                "the contract evaluates caller-supplied capacity measurements and does not schedule, execute, or move raw research data",
                "a BioethicsCapacityReport2 is a compatibility artifact, not a safety certification, scientific conclusion, or clinical advice"
            ]
        }))
    }

    pub(super) fn bioethics_multimodal_context_compilation_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a Bioethics DecisionQuery2")?;
        let receipt =
            crate::research_contracts::run_bioethics_multimodal_context_compilation_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_bioethics::MULTIMODAL_CONTEXT_COMPILATION_FEATURE_ID,
            "contract_version": bioprism_bioethics::MULTIMODAL_CONTEXT_COMPILATION_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "multi-study multimodal context facts are checked for ethical review, scope, semantic profile, evidence, replay, provenance, and protected closure",
                "missing studies/modalities, uncertainty, negative results, omission reasons, and ethical gates remain explicit",
                "human-subject and clinical-source data are outside the preclinical boundary and unsafe states fail closed"
            ],
            "limitations": [
                "the harness consumes caller-supplied metadata and does not infer facts or render a clinical decision section",
                "a qualified context receipt is not diagnosis, treatment, triage, enrollment, or clinical advice"
            ]
        }))
    }

    pub(super) fn bioethics_statistical_analysis_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a Bioethics AnalysisQuestion3")?;
        let receipt =
            crate::research_contracts::run_bioethics_statistical_analysis_assurance_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_bioethics::STATISTICAL_ANALYSIS_ASSURANCE_FEATURE_ID,
            "contract_version": bioprism_bioethics::STATISTICAL_ANALYSIS_ASSURANCE_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "analysis declarations are checked for estimand, scope, semantic profile, quality, evidence, replay, provenance, privacy, and dual-use review",
                "negative, uncertain, omitted, policy, protected-closure, and institutional-authorization states remain explicit",
                "the harness is verification-only and never fits a model or reads raw arrays"
            ],
            "limitations": [
                "the harness consumes caller-supplied metadata and does not execute statistical, causal, or ML analysis",
                "a qualified analysis receipt is not a scientific conclusion or clinical advice"
            ]
        }))
    }

    pub(super) fn bioethics_experiment_design_workflow_fabric(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an ExperimentDesignWorkflowRequest1")?;
        let receipt =
            crate::research_contracts::run_bioethics_experiment_design_workflow_fabric_json(
                request,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_bioethics::EXPERIMENT_DESIGN_WORKFLOW_FEATURE_ID,
            "contract_version": bioprism_bioethics::EXPERIMENT_DESIGN_WORKFLOW_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "experiment design steps are dependency-checked and canonically scheduled with cycle witnesses",
                "power, evidence, policy, approval, budget, protected-closure, replay, and locality gates remain explicit",
                "the fabric emits only a bounded schedule receipt and never executes instruments or physical work"
            ],
            "limitations": [
                "the fabric compiles caller-supplied objective metadata and does not perform power calculations",
                "a qualified schedule is not diagnosis, treatment, triage, enrollment, or clinical advice"
            ]
        }))
    }

    pub(super) fn bioethics_multimodal_bounded_evolution_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt =
            crate::research_contracts::run_bioethics_multimodal_bounded_evolution_assurance_json(
                arguments,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_bioethics::MULTIMODAL_BOUNDED_EVOLUTION_FEATURE_ID,
            "contract_version": bioprism_bioethics::MULTIMODAL_BOUNDED_EVOLUTION_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "multimodal candidate evolution is partitioned by study/modality closure, compatibility, benchmark, ethics, privacy, dual-use, provenance, replay, policy, locality, and protected-closure gates",
                "unknown, contradicted, omitted, negative-result, incomparability, benchmark, and ethics failures remain explicit",
                "the A1 assurance harness never mutates implementations, grants authority, exports raw data, or makes clinical decisions"
            ],
            "limitations": [
                "the harness evaluates caller-supplied attestations and does not run benchmarks or deploy an evolution",
                "a qualified observation receipt is not scientific validity, diagnosis, treatment, triage, enrollment, or clinical advice"
            ]
        }))
    }

    pub(super) fn bioethics_action_review(&self, arguments: &Value) -> Result<Value, String> {
        let raw_plan = arguments
            .get("plan")
            .cloned()
            .ok_or("plan is required and must be a serialized bioprism-bioethics ActionPlan")?;
        let plan: ActionPlan = serde_json::from_value(raw_plan)
            .map_err(|error| format!("invalid action plan: {error}"))?;
        if plan.steps.len() > 10_000 {
            return Err("plan may contain at most 10000 steps".into());
        }

        let boundary = match arguments.get("boundary") {
            Some(raw) => serde_json::from_value(raw.clone())
                .map_err(|error| format!("invalid research boundary: {error}"))?,
            None => ResearchBoundary::research_only(),
        };
        let disposition = match plan.partition(&boundary) {
            Ok(disposition) => disposition,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "stage": "research_boundary",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "guarantee": "a clinical use refuses the complete plan before physical or in-silico steps are separated"
                }));
            }
        };

        let physical_step_count = disposition.physical_steps().len();
        let in_silico_step_count = disposition.in_silico_steps().len();
        let referral = if disposition.requires_physical_authorisation() {
            match arguments.get("authorisation") {
                None => json!({
                    "status": "not_attempted",
                    "refusal": "physical steps require both human approval and institutional safety review before referral",
                    "fail_closed": true,
                }),
                Some(raw) => {
                    let authorisation: Authorisation = serde_json::from_value(raw.clone())
                        .map_err(|error| format!("invalid action authorisation: {error}"))?;
                    match refer_physical_action(&disposition, &authorisation) {
                        Ok(referral) => json!({
                            "status": "referred",
                            "referral": referral,
                            "statement": bioprism_bioethics::action::PhysicalReferral::STATEMENT,
                            "executes_physical_action": false,
                        }),
                        Err(error) => json!({
                            "status": "refused",
                            "refusal": error.to_string(),
                            "fail_closed": true,
                            "executes_physical_action": false,
                        }),
                    }
                }
            }
        } else {
            json!({
                "status": "not_required",
                "refusal": null,
                "executes_physical_action": false,
            })
        };

        Ok(json!({
            "ok": true,
            "subject": disposition.subject(),
            "declared_use": plan.declared_use,
            "permitted_uses": boundary.permitted(),
            "disposition": disposition,
            "physical_step_count": physical_step_count,
            "in_silico_step_count": in_silico_step_count,
            "requires_external_authorisation": physical_step_count > 0,
            "referral": referral,
            "guarantees": [
                "clinical-use refusal happens before plan partitioning",
                "physical actions are only represented as a referral and are never executed",
                "missing or incomplete human authorisation remains a typed refusal",
            ],
        }))
    }

    pub(super) fn bioethics_human_subject_screen(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let raw_study = arguments
            .get("study")
            .cloned()
            .ok_or("study is required and must be a serialized StudyDescription")?;
        let study: StudyDescription = serde_json::from_value(raw_study)
            .map_err(|error| format!("invalid study description: {error}"))?;
        if study.engagements.len() > 100 {
            return Err("study may declare at most 100 engagement kinds".into());
        }

        let determination = screen_human_subject(&study);
        let consent_check = match (arguments.get("consent"), arguments.get("at")) {
            (None, None) => json!({
                "status": "not_run",
                "reason": "provide both consent and at to check every declared purpose",
            }),
            (Some(_), None) | (None, Some(_)) => {
                return Err("consent and at must be supplied together".into());
            }
            (Some(raw_consent), Some(raw_at)) => {
                let consent: bioprism_policy::Consent = serde_json::from_value(raw_consent.clone())
                    .map_err(|error| format!("invalid consent: {error}"))?;
                let at_text = raw_at
                    .as_str()
                    .ok_or("at must be an RFC-3339 timestamp string")?;
                let at = bioprism_scope::Timestamp::parse(at_text)
                    .map_err(|error| format!("invalid at timestamp: {error}"))?;
                match study.check_consent(&consent, at) {
                    Ok(()) => json!({ "status": "admitted", "at": at_text }),
                    Err(error) => json!({
                        "status": "refused",
                        "at": at_text,
                        "refusal": error.to_string(),
                        "fail_closed": true,
                    }),
                }
            }
        };

        let boundary = match arguments.get("boundary") {
            Some(raw) => serde_json::from_value(raw.clone())
                .map_err(|error| format!("invalid research boundary: {error}"))?,
            None => ResearchBoundary::research_only(),
        };
        let return_of_results = match study.check_return_of_results(&boundary) {
            Ok(()) => json!({ "status": "admitted" }),
            Err(error) => json!({
                "status": "refused",
                "refusal": error.to_string(),
                "fail_closed": true,
            }),
        };

        Ok(json!({
            "ok": true,
            "subject": study.subject,
            "determination": determination,
            "requires_institutional_review": determination.requires_review(),
            "triggers": determination.triggers(),
            "consent": consent_check,
            "return_of_results": return_of_results,
            "clearance_issued": false,
            "guarantees": [
                "no-engagement screening is undetermined, never an exemption",
                "consent is checked against the policy crate's closed purpose vocabulary",
                "individual findings remain subject to the research-only boundary",
            ],
        }))
    }

    pub(super) fn bioethics_dual_use_review(&self, arguments: &Value) -> Result<Value, String> {
        let raw_release = arguments
            .get("release")
            .cloned()
            .ok_or("release is required and must be a serialized CapabilityRelease")?;
        let release: CapabilityRelease = serde_json::from_value(raw_release)
            .map_err(|error| format!("invalid capability release: {error}"))?;
        let raw_risk = arguments
            .get("risk")
            .cloned()
            .ok_or("risk is required and must be a serialized RiskAssessment")?;
        let risk: RiskAssessment = serde_json::from_value(raw_risk)
            .map_err(|error| format!("invalid risk assessment: {error}"))?;

        let referral = match refer_dual_use(&release, &risk) {
            Ok(referral) => referral,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "stage": "dual_use_release",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "guarantee": "unassessed misuse, subject mismatch, missing category, or unrated safety dimensions do not release"
                }));
            }
        };

        let withholding = match arguments.get("withhold") {
            None => json!({ "status": "not_requested" }),
            Some(raw) => {
                let scope: bioprism_safety::release::WithholdScope =
                    serde_json::from_value(raw.clone())
                        .map_err(|error| format!("invalid withhold scope: {error}"))?;
                let finding = arguments
                    .get("finding")
                    .and_then(Value::as_str)
                    .unwrap_or("caller-supplied finding");
                match referral.withhold(finding, scope) {
                    Ok(scope) => json!({ "status": "admitted", "scope": scope }),
                    Err(error) => json!({
                        "status": "refused",
                        "scope": scope,
                        "refusal": error.to_string(),
                        "fail_closed": true,
                    }),
                }
            }
        };

        Ok(json!({
            "ok": true,
            "subject": referral.subject(),
            "surfaces": referral.surfaces(),
            "assessor": referral.assessor(),
            "sensitive_category": referral.category(),
            "decision": referral.decision(),
            "referral": referral,
            "withholding": withholding,
            "guarantees": [
                "an assessed empty surface set is distinct from no assessment",
                "the section 13 risk gate is reused without a second dual-use rule",
                "suppressing exploit detail remains distinct from suppressing the existence of a finding",
            ],
        }))
    }

    pub(super) fn bioethics_validation_check(&self, arguments: &Value) -> Result<Value, String> {
        let raw_dossier = arguments
            .get("dossier")
            .cloned()
            .ok_or("dossier is required and must be a serialized ValidationDossier")?;
        let raw_bytes = serde_json::to_vec(&raw_dossier).map_err(|error| error.to_string())?;
        if raw_bytes.len() > 10_000_000 {
            return Err("validation dossier exceeds the 10000000-byte safety bound".into());
        }
        let dossier: ValidationDossier = serde_json::from_value(raw_dossier)
            .map_err(|error| format!("invalid validation dossier: {error}"))?;
        let missing = dossier.missing();
        let verification = match dossier.verify() {
            Ok(verified) => json!({
                "status": "verified",
                "verified_module": verified,
            }),
            Err(error) => json!({
                "status": "refused",
                "refusal": error.to_string(),
                "fail_closed": true,
            }),
        };
        Ok(json!({
            "ok": true,
            "subject": dossier.subject,
            "author": dossier.author,
            "maturity": dossier.maturity(),
            "missing": missing,
            "missing_count": missing.len(),
            "verification": verification,
            "guarantees": [
                "blank or absent evidence is not treated as satisfied",
                "independent reproduction must name an actor other than the author",
                "verified status is minted only by the in-process validation check",
            ],
        }))
    }

    pub(super) fn bioethics_representation_audit(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let subject = arguments
            .get("subject")
            .and_then(Value::as_str)
            .ok_or("subject is required")?;
        let raw_observations = arguments
            .get("observations")
            .and_then(Value::as_array)
            .ok_or("observations is required and must be an array")?;
        if raw_observations.len() > 10_000 {
            return Err("observations may contain at most 10000 strata".into());
        }
        let observations = raw_observations
            .iter()
            .enumerate()
            .map(|(index, raw)| {
                serde_json::from_value::<StratumObservation>(raw.clone())
                    .map_err(|error| format!("invalid observation at index {index}: {error}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let summary = match summarise(subject, observations) {
            Ok(summary) => summary,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "stage": "representation_partition",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                }));
            }
        };

        let attribution = match arguments.get("attribution") {
            None => json!({ "status": "not_requested" }),
            Some(raw) => {
                let object = raw.as_object().ok_or("attribution must be an object")?;
                let axis: ContextAxis = serde_json::from_value(
                    object
                        .get("axis")
                        .cloned()
                        .ok_or("attribution.axis is required")?,
                )
                .map_err(|error| format!("invalid attribution axis: {error}"))?;
                let raw_matched = object
                    .get("matched")
                    .and_then(Value::as_array)
                    .ok_or("attribution.matched is required and must be an array")?;
                let matched = raw_matched
                    .iter()
                    .cloned()
                    .map(|item| {
                        serde_json::from_value::<ContextAxis>(item)
                            .map_err(|error| format!("invalid matched context axis: {error}"))
                    })
                    .collect::<Result<BTreeSet<_>, _>>()?;
                let finding = object
                    .get("finding")
                    .and_then(Value::as_str)
                    .ok_or("attribution.finding is required")?;
                let decision = attribute_representation(axis, &matched);
                let standing = match decision.clone().require_context(finding) {
                    Ok(axis) => json!({ "status": "admitted", "axis": axis }),
                    Err(error) => json!({
                        "status": "refused",
                        "refusal": error.to_string(),
                        "fail_closed": true,
                    }),
                };
                json!({ "status": "evaluated", "decision": decision, "standing": standing })
            }
        };

        Ok(json!({
            "ok": true,
            "summary": summary,
            "measured_count": summary.measured().len(),
            "unmeasured_count": summary.unmeasured().len(),
            "suppressed_count": summary.suppressed().len(),
            "complete": summary.is_complete(),
            "incomplete_axes": summary.incomplete_axes(),
            "attribution": attribution,
            "guarantees": [
                "measured, unmeasured, and small-cell-suppressed strata remain separate",
                "duplicate strata refuse rather than overwrite one another",
                "resource-context mismatch blocks attribution to a demographic or geographic axis",
            ],
        }))
    }
}
