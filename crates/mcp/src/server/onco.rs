//! MCP Oncology policy, provenance, and execution handlers.
//!
//! Kept beside the dispatcher so each domain surface can be reviewed independently.

use super::*;

impl Server {
    pub(super) fn onco_federated_provenance_signing(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt = crate::research_contracts::operate_onco_federated_provenance_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": FEDERATED_PROVENANCE_FEATURE_ID,
            "contract_version": FEDERATED_PROVENANCE_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "artifact, site, signer, and provenance-lineage partitions are deterministic and complete",
                "stale, revoked, contradictory, unknown, speculative, omitted, negative, replay, semantic, purpose, policy, closure, federation, locality, and adversarial states remain observable",
                "signature and release gates fail closed; only digest-bound signed-provenance exchange can qualify",
                "raw preclinical OncoWorld data remains local and the route never diagnoses, treats, triages, enrolls, or makes clinical decisions"
            ],
            "limitations": [
                "the fabric verifies caller-supplied signer and provenance declarations and does not contact key registries or fetch artifact bytes",
                "a SignedProvenanceWorkflow9 is a provenance exchange gate, not cryptographic proof of scientific validity or a clinical conclusion"
            ]
        }))
    }

    pub(super) fn onco_instrument_research_workbench(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt =
            crate::research_contracts::operate_onco_instrument_research_workbench_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": ONCO_INSTRUMENT_FEATURE_ID,
            "contract_version": ONCO_INSTRUMENT_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "OncoWorld instrument actions are deterministically ordered and partitioned into selected, unresolved, blocked, and missing states",
                "worldline/specimen scope, protocol readiness, preclinical consent, replay, evidence, provenance, policy, protected-closure, signed-approval, researcher authority, locality, and adversarial gates fail closed",
                "qualified output is a read-only researcher interaction receipt; the workbench never contacts hardware, dispatches actions, moves raw data, or makes clinical decisions"
            ],
            "limitations": [
                "the workbench evaluates caller-supplied instrument and worldline attestations and does not execute protocols or authenticate hardware",
                "an OncoInstrumentReceipt5 is a compatibility and safety-review artifact, not an assay result, treatment recommendation, or clinical conclusion"
            ]
        }))
    }

    pub(super) fn onco_computational_execution_contract_model(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a ResearchWorkflowSpec1")?;
        let receipt =
            crate::research_contracts::run_onco_computational_execution_contract_model_json(
                request,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_onco::COMPUTATIONAL_EXECUTION_FEATURE_ID,
            "contract_version": bioprism_onco::COMPUTATIONAL_EXECUTION_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "typed execution graphs require unique dependencies, replay and provenance digests, deterministic nodes, and local-only metadata",
                "dependency cycles, non-determinism, policy gaps, and locality failures remain explicit in the run contract",
                "the model is A0 and never dispatches computation, instruments, or clinical actions"
            ],
            "limitations": [
                "the contract model validates caller-supplied graph metadata and does not execute nodes",
                "a qualified run contract is not diagnosis, treatment, triage, enrollment, or clinical advice"
            ]
        }))
    }

    pub(super) fn onco_boundary_check(&self, arguments: &Value) -> Result<Value, String> {
        let boundary: ResearchBoundary = arguments
            .get("boundary")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| format!("invalid research boundary: {error}"))?
            .unwrap_or_else(ResearchBoundary::research_only);
        let request: BoundaryRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or("request is required and must be a serialized BoundaryRequest")?,
        )
        .map_err(|error| format!("invalid boundary request: {error}"))?;
        if request.requested_uses.len() > 100 {
            return Err("boundary request exceeds the 100-use safety bound".into());
        }

        let disposition = match boundary.triage(&request) {
            Ok(disposition) => disposition,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/onco-boundary-check/0.1",
                    "outcome_kind": "refused",
                    "refusal_kind": "identifiers_present",
                    "stage": "research_boundary",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "requested_use_count": request.requested_uses.len(),
                    "identifier_fields_present": true,
                    "guarantee": "direct identifier presence is refused before the request is echoed or analysed"
                }));
            }
        };
        let disposition_value =
            serde_json::to_value(&disposition).map_err(|error| error.to_string())?;
        let disposition_kind = disposition_value["disposition"]
            .as_str()
            .ok_or("boundary disposition must carry a tagged disposition")?;
        let escalation = disposition.escalation();
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/onco-boundary-check/0.1",
            "outcome_kind": "disposition",
            "disposition_kind": disposition_kind,
            "permitted": boundary.permitted(),
            "disposition": disposition,
            "released": disposition.released(),
            "refused": disposition.refused(),
            "terminal_action": disposition.terminal_action(),
            "escalation": escalation,
            "escalation_present": escalation.is_some(),
            "escalation_trigger": escalation.map(|notice| notice.trigger()),
            "escalation_route": escalation.map(|notice| notice.route()),
            "requested_use_count": disposition.released().len() + disposition.refused().len(),
            "released_count": disposition.released().len(),
            "refused_count": disposition.refused().len(),
            "identifier_fields_present": false,
            "research_statement": "Research use only. Not for use in the diagnosis, prognosis, treatment, or triage of any individual.",
            "guarantees": [
                "research uses and individualized clinical uses are split rather than collapsed into a total refusal",
                "claimed role, urgency, and context do not override the fixed research boundary",
                "partial release preserves safe aggregate work while routing refused individual use to a human process"
            ],
            "limitations": [
                "the boundary checks declared uses and identifier fields; it does not inspect embedded instructions in arbitrary source text",
                "a boundary disposition is not clinical advice and does not execute escalation"
            ]
        }))
    }

    pub(super) fn onco_response_assess(&self, arguments: &Value) -> Result<Value, String> {
        let criterion: ResponseCriterion = serde_json::from_value(
            arguments
                .get("criterion")
                .cloned()
                .ok_or("criterion is required and must be a serialized ResponseCriterion")?,
        )
        .map_err(|error| format!("invalid response criterion: {error}"))?;
        let baseline: ImagingObservation = serde_json::from_value(
            arguments
                .get("baseline")
                .cloned()
                .ok_or("baseline is required and must be an ImagingObservation")?,
        )
        .map_err(|error| format!("invalid baseline imaging observation: {error}"))?;
        let current: ImagingObservation = serde_json::from_value(
            arguments
                .get("current")
                .cloned()
                .ok_or("current is required and must be an ImagingObservation")?,
        )
        .map_err(|error| format!("invalid current imaging observation: {error}"))?;
        let current_acquired: AcquisitionTime = serde_json::from_value(
            arguments
                .get("current_acquired")
                .cloned()
                .ok_or("current_acquired is required and must be an RFC-3339 timestamp")?,
        )
        .map_err(|error| format!("invalid current_acquired timestamp: {error}"))?;
        let baseline_clinical: ClinicalObservation = serde_json::from_value(
            arguments
                .get("baseline_clinical")
                .cloned()
                .ok_or("baseline_clinical is required and must be a ClinicalObservation")?,
        )
        .map_err(|error| format!("invalid baseline clinical observation: {error}"))?;
        let current_clinical: ClinicalObservation = serde_json::from_value(
            arguments
                .get("current_clinical")
                .cloned()
                .ok_or("current_clinical is required and must be a ClinicalObservation")?,
        )
        .map_err(|error| format!("invalid current clinical observation: {error}"))?;
        let treatment: TreatmentContext = serde_json::from_value(
            arguments
                .get("treatment")
                .cloned()
                .ok_or("treatment is required and must be a TreatmentContext")?,
        )
        .map_err(|error| format!("invalid treatment context: {error}"))?;
        let evidence: ProgressionEvidence = arguments
            .get("evidence")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| format!("invalid progression evidence: {error}"))?
            .unwrap_or_default();
        let nadir_spd_mm2 = match arguments.get("nadir_spd_mm2") {
            None | Some(Value::Null) => None,
            Some(value) => Some(
                value
                    .as_f64()
                    .ok_or("nadir_spd_mm2 must be a finite number or null")?,
            ),
        };
        let measurement_error_fraction = arguments
            .get("measurement_error_fraction")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);

        let request = ResponseRequest {
            criterion: &criterion,
            baseline: &baseline,
            current: &current,
            current_acquired,
            nadir_spd_mm2,
            baseline_clinical: &baseline_clinical,
            current_clinical: &current_clinical,
            treatment: &treatment,
            evidence: &evidence,
            measurement_error_fraction,
        };
        let assessment = match onco_assess(&request) {
            Ok(assessment) => assessment,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/onco-response-assess/0.1",
                    "outcome_kind": "refused",
                    "refusal_kind": "assessment_error",
                    "stage": "response_assessment",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "criterion": criterion,
                    "treatment": treatment,
                    "evidence_present": arguments.get("evidence").is_some(),
                    "guarantee": "invalid or unsupported measurements never become stable, response, or progression calls"
                }));
            }
        };
        let assessment_value =
            serde_json::to_value(&assessment).map_err(|error| error.to_string())?;
        let call_kind = assessment_value["call"]["call"]
            .as_str()
            .ok_or("response assessment must carry a tagged call")?;
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/onco-response-assess/0.1",
            "outcome_kind": "assessment",
            "call_kind": call_kind,
            "unconfirmed_reading": assessment.unconfirmed_reading,
            "criterion": criterion,
            "treatment": treatment,
            "criterion_recognises_post_treatment_change": criterion.recognises_post_treatment_change,
            "post_treatment_window_days": treatment.modality.post_treatment_window_days(),
            "pseudoresponse_possible": treatment.modality.causes_pseudoresponse(),
            "measurement_error_fraction": measurement_error_fraction,
            "evidence_present": arguments.get("evidence").is_some(),
            "criterion_divergence_present": assessment.divergence_from_criterion.is_some(),
            "sensitivity_flips": assessment.sensitivity.flips_within_measurement_error,
            "hypothesis_non_identifiable": assessment.hypotheses.is_non_identifiable(),
            "assessment": assessment,
            "call_label": assessment.call.label(),
            "withheld_progression": assessment.withheld_progression(),
            "hypothesis_count": assessment.hypotheses.entries().len(),
            "evidence_requests": assessment.hypotheses.evidence_requests(),
            "guarantees": [
                "the unconfirmed radiologic reading is separate from the reportable call",
                "post-treatment change without confirmation remains not evaluable and never becomes stable disease",
                "the surviving differential and discriminating evidence requests remain visible"
            ],
            "limitations": [
                "this is a research-world assessment, not a diagnosis, prognosis, treatment recommendation, or care triage",
                "advanced imaging and calibrated probabilities are not inferred when they are absent from the request"
            ]
        }))
    }

    pub(super) fn onco_worldline_view(&self, arguments: &Value) -> Result<Value, String> {
        let worldline: TumourWorldline = serde_json::from_value(
            arguments
                .get("worldline")
                .cloned()
                .ok_or("worldline is required and must be a serialized TumourWorldline")?,
        )
        .map_err(|error| format!("invalid tumour worldline: {error}"))?;
        if worldline.timepoints().len() > 100_000 {
            return Err("tumour worldline exceeds the 100000-timepoint safety bound".into());
        }

        let cutoff = match arguments.get("visible_at") {
            None | Some(Value::Null) => None,
            Some(value) => {
                let text = value
                    .as_str()
                    .ok_or("visible_at must be an RFC-3339 string or null")?;
                let timestamp = bioprism_scope::Timestamp::parse(text)
                    .map_err(|error| format!("invalid visible_at timestamp: {error}"))?;
                Some(AvailabilityTime::new(timestamp))
            }
        };
        let biological_order: Vec<&str> = worldline
            .timepoints()
            .iter()
            .map(Timepoint::label)
            .collect();
        let record_order: Vec<&str> = worldline
            .in_record_order()
            .into_iter()
            .map(Timepoint::label)
            .collect();
        let visible = cutoff.map(|at| worldline.visible_at(at));
        let visible_labels = visible.as_ref().map(|timepoints| {
            timepoints
                .iter()
                .map(|timepoint| timepoint.label())
                .collect::<Vec<_>>()
        });
        let hidden_labels = cutoff.map(|at| {
            worldline
                .timepoints()
                .iter()
                .filter(|timepoint| timepoint.visible() > at)
                .map(Timepoint::label)
                .collect::<Vec<_>>()
        });
        let record_index = |label: &str| {
            record_order
                .iter()
                .position(|record_label| *record_label == label)
                .expect("every biological timepoint must have a record-order position")
        };
        let rows: Vec<Value> = worldline
            .timepoints()
            .iter()
            .enumerate()
            .map(|(biological_index, timepoint)| {
                let clocks = timepoint.clocks();
                let visible_at_cutoff = cutoff.map(|at| clocks.visible <= at);
                let visibility_state = match visible_at_cutoff {
                    Some(true) => "visible",
                    Some(false) => "hidden_from_agent",
                    None => "not_filtered",
                };
                json!({
                    "label": timepoint.label(),
                    "biological_index": biological_index,
                    "record_index": record_index(timepoint.label()),
                    "clocks": clocks,
                    "acquired": clocks.acquired,
                    "recorded": clocks.recorded,
                    "released": clocks.released,
                    "visible": clocks.visible,
                    "days_from_baseline": worldline.time_from_baseline(timepoint).days(),
                    "observation": timepoint.observation(),
                    "visibility_state": visibility_state,
                    "visible_at_cutoff": visible_at_cutoff,
                })
            })
            .collect();

        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/onco-worldline-view/0.1",
            "subject": worldline.subject().as_str(),
            "baseline": worldline.baseline().label(),
            "timepoint_count": worldline.timepoints().len(),
            "biological_order": biological_order,
            "record_order": record_order,
            "record_order_differs": biological_order != record_order,
            "clock_axes": ["acquired", "recorded", "released", "visible"],
            "clock_order_guaranteed": true,
            "baseline_biological_index": worldline
                .timepoints()
                .iter()
                .position(|timepoint| timepoint.label() == worldline.baseline().label())
                .expect("the worldline baseline must be present"),
            "baseline_record_index": record_index(worldline.baseline().label()),
            "visibility_cutoff": cutoff.map(|at| at.timestamp().to_rfc3339()),
            "visibility_filter_applied": cutoff.is_some(),
            "visible_timepoints": visible_labels,
            "hidden_from_agent": hidden_labels,
            "visibility_partition": {
                "cutoff": cutoff.map(|at| at.timestamp().to_rfc3339()),
                "filter_applied": cutoff.is_some(),
                "visible": visible.as_ref().map(|timepoints| timepoints.iter().map(|timepoint| timepoint.label()).collect::<Vec<_>>()),
                "hidden": cutoff.map(|at| worldline.timepoints().iter().filter(|timepoint| timepoint.visible() > at).map(Timepoint::label).collect::<Vec<_>>()),
                "visible_count": visible.as_ref().map(Vec::len),
                "hidden_count": hidden_labels.as_ref().map(Vec::len),
            },
            "visible_count": visible.as_ref().map(Vec::len),
            "hidden_count": hidden_labels.as_ref().map(Vec::len),
            "timepoints": rows,
            "guarantees": [
                "biological order is acquisition order and is reported separately from record order",
                "visibility is cut on agent-visibility time rather than acquisition time",
                "the baseline-relative day count uses only the acquisition clock",
                "hidden future evidence remains distinguishable from absent or unrecorded evidence"
            ],
            "limitations": [
                "the tool audits and renders a supplied worldline; it does not infer missing observations or repair clocks",
                "the worldline contains exact timestamps and does not model date uncertainty, treatment interruptions, or specimen lineage"
            ]
        }))
    }

    pub(super) fn onco_classification_check(&self, arguments: &Value) -> Result<Value, String> {
        let histology: Histology = serde_json::from_value(
            arguments
                .get("histology")
                .cloned()
                .ok_or("histology is required and must be a serialized Histology")?,
        )
        .map_err(|error| format!("invalid histology: {error}"))?;
        let panel: MarkerPanel = serde_json::from_value(
            arguments
                .get("panel")
                .cloned()
                .ok_or("panel is required and must be a serialized MarkerPanel")?,
        )
        .map_err(|error| format!("invalid molecular marker panel: {error}"))?;
        let states: Vec<Value> = panel
            .iter()
            .map(|(marker, state)| json!({ "marker": marker, "state": state }))
            .collect();
        if states.len() > 100 {
            return Err("molecular marker panel exceeds the 100-marker safety bound".into());
        }
        let resolution = onco_classify(histology, &panel);
        let resolution_value =
            serde_json::to_value(&resolution).map_err(|error| error.to_string())?;
        let resolution_kind = resolution_value
            .get("resolution")
            .and_then(Value::as_str)
            .ok_or("classification resolution must carry a tagged resolution kind")?;
        let obligation_count = resolution.obligations().len();
        let observed_panel_state_count = panel
            .iter()
            .filter(|(_, state)| state.is_observed())
            .count();
        let unobserved_panel_state_count = states.len().saturating_sub(observed_panel_state_count);
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/onco-classification-check/0.1",
            "histology": histology,
            "resolution": resolution_value,
            "resolution_kind": resolution_kind,
            "is_integrated": resolution.is_integrated(),
            "entity": resolution.entity(),
            "obligations": resolution.obligations(),
            "obligation_count": obligation_count,
            "panel_states": states,
            "panel_state_count": states.len(),
            "observed_panel_state_count": observed_panel_state_count,
            "unobserved_panel_state_count": unobserved_panel_state_count,
            "guarantees": [
                "histology selects candidates but never supplies a molecular call",
                "unobserved assays remain obligations and are never treated as negative",
                "mixed, unresolved, provisional, and not-otherwise-resolved states remain distinct",
                "an entity is returned only for an integrated resolution"
            ],
            "limitations": [
                "the criteria table is the deliberately bounded worked instantiation shipped by bioprism-onco",
                "methylation, fusion, purity, and lower-grade histologic evidence are not inferred"
            ]
        }))
    }

    pub(super) fn onco_outcome_analyze(&self, arguments: &Value) -> Result<Value, String> {
        let follow_up: FollowUp = serde_json::from_value(
            arguments
                .get("follow_up")
                .cloned()
                .ok_or("follow_up is required and must be a serialized FollowUp")?,
        )
        .map_err(|error| format!("invalid oncology follow-up: {error}"))?;
        let estimand: Estimand = serde_json::from_value(
            arguments
                .get("estimand")
                .cloned()
                .ok_or("estimand is required and must be declared before analysis")?,
        )
        .map_err(|error| format!("invalid oncology estimand: {error}"))?;
        let analysis = follow_up.analyse(&estimand);
        let at_risk_days = analysis.at_risk_days;
        let immortal_time_days = analysis.immortal_time_days;
        let event = analysis.outcome.is_event();
        let censoring_reason = analysis.outcome.censoring_reason();
        let informative_bias_flags = analysis
            .bias_flags
            .iter()
            .filter(|bias| {
                matches!(
                    bias,
                    bioprism_onco::AnalysisBias::InformativeLossToFollowUp
                        | bioprism_onco::AnalysisBias::CompetingDeath
                        | bioprism_onco::AnalysisBias::TreatmentSwitching
                )
            })
            .collect::<Vec<_>>();
        let all_bias_flags = analysis.bias_flags.clone();
        let bias_count = all_bias_flags.len();
        let informative_bias_count = informative_bias_flags.len();
        let outcome = analysis.outcome;
        let censoring_informative =
            censoring_reason.map(|reason| reason.is_potentially_informative());
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/onco-outcome-analyze/0.1",
            "analysis": analysis,
            "outcome": outcome,
            "bias_flags": all_bias_flags,
            "bias_count": bias_count,
            "informative_bias_count": informative_bias_count,
            "at_risk_days": at_risk_days,
            "immortal_time_days": immortal_time_days,
            "left_truncated": immortal_time_days > 0,
            "event": event,
            "censoring_reason": censoring_reason,
            "censoring_informative": censoring_informative,
            "informative_bias_flags": informative_bias_flags,
            "guarantees": [
                "the caller must state the estimand before the per-subject outcome is interpreted",
                "loss to follow-up is censoring and never an event",
                "competing death remains distinguishable from ordinary non-informative censoring",
                "delayed risk-set entry reports immortal-time exposure instead of counting it as at-risk time"
            ],
            "limitations": [
                "this produces one-subject analysis records; it does not estimate survival, hazard ratios, or cumulative incidence",
                "cohort-level landmark and follow-up-intensity bias require a cohort audit beyond this record"
            ]
        }))
    }
}
