//! MCP OncoWorld identity, transport, and evidence handlers.
//!
//! Kept beside the dispatcher so each domain surface can be reviewed independently.

use super::*;

impl Server {
    pub(super) fn oncoworlds_identity_join(&self, arguments: &Value) -> Result<Value, String> {
        let left: Artifact = serde_json::from_value(
            arguments
                .get("left")
                .cloned()
                .ok_or("left is required and must be a serialized onco-worlds Artifact")?,
        )
        .map_err(|error| format!("invalid left artifact: {error}"))?;
        let right: Artifact = serde_json::from_value(
            arguments
                .get("right")
                .cloned()
                .ok_or("right is required and must be a serialized onco-worlds Artifact")?,
        )
        .map_err(|error| format!("invalid right artifact: {error}"))?;
        let unit: AnalysisUnit = serde_json::from_value(arguments.get("unit").cloned().ok_or(
            "unit is required and must be participant, lesion, specimen, or imaging_series",
        )?)
        .map_err(|error| format!("invalid analysis unit: {error}"))?;
        let evidence: IdentityEvidence = arguments
            .get("evidence")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| format!("invalid identity evidence: {error}"))?
            .unwrap_or_else(IdentityEvidence::new);
        if evidence.links().len() > 1_000 {
            return Err("identity evidence exceeds the 1000-link safety bound".into());
        }
        let bridge: Option<EpochBridge> = arguments
            .get("epoch_bridge")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| format!("invalid epoch bridge: {error}"))?;
        let verdict = match bridge.as_ref() {
            Some(bridge) => joinable_with_bridge(&left, &right, unit, &evidence, Some(bridge)),
            None => joinable_with_bridge(&left, &right, unit, &evidence, None),
        };
        let verdict = match verdict {
            Ok(()) => JoinVerdict::Joinable,
            Err(reason) => JoinVerdict::Declined { reason },
        };
        let report = JoinReport {
            left: left.label.clone(),
            right: right.label.clone(),
            unit,
            verdict,
        };
        let report_value = serde_json::to_value(&report).map_err(|error| error.to_string())?;
        let verdict_kind = report_value["verdict"]["verdict"]
            .as_str()
            .ok_or("identity join report must carry a tagged verdict")?;
        let refusal_kind = report_value["verdict"]["reason"]["refusal"]
            .as_str()
            .map(str::to_owned);
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/oncoworlds-identity-join/0.1",
            "joinable": report.verdict.is_joinable(),
            "report": report,
            "verdict_kind": verdict_kind,
            "refusal_kind": refusal_kind,
            "bridge_declared": bridge.is_some(),
            "epoch_bridge": bridge,
            "identity_evidence_present": !evidence.links().is_empty(),
            "identity_link_count": evidence.links().len(),
            "bridge_warrant_present": bridge.as_ref().is_some_and(|item| !item.warrant.is_empty()),
            "checked_dimensions": [
                "participant_identity",
                "identifier_width",
                "identity_evidence",
                "relation_licence",
                "permissible_use",
                "lesion_identity",
                "disease_epoch",
                "specimen_identity"
            ],
            "guarantees": [
                "participant identity is checked before local specimen or lesion identifiers",
                "truncated identifiers, unlicensed relations, missing permissible use, epoch mismatch, and specimen mismatch are typed refusals",
                "a declined join is returned as an auditable result rather than discarded",
                "an epoch bridge is accepted only when it names both epochs and carries a caller-supplied warrant"
            ],
            "limitations": [
                "the tool consumes caller-supplied identity evidence and does not run fingerprint, sex-chromosome, or contamination oracles",
                "a declared bridge is a warrant for review, not proof that the disease epochs are biologically interchangeable"
            ]
        }))
    }

    pub(super) fn oncoworlds_model_transport(&self, arguments: &Value) -> Result<Value, String> {
        let result: ModelResult = serde_json::from_value(
            arguments
                .get("result")
                .cloned()
                .ok_or("result is required and must be a serialized ModelResult")?,
        )
        .map_err(|error| format!("invalid model result: {error}"))?;
        let fidelity: FidelityEvidence = arguments
            .get("fidelity")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| format!("invalid fidelity evidence: {error}"))?
            .unwrap_or_default();
        let cohort: EstablishmentCohort = serde_json::from_value(
            arguments
                .get("establishment")
                .cloned()
                .ok_or("establishment is required and must be an EstablishmentCohort")?,
        )
        .map_err(|error| format!("invalid establishment cohort: {error}"))?;
        if cohort.attempted > 1_000_000 || cohort.established > 1_000_000 {
            return Err("establishment counts exceed the 1000000-subject safety bound".into());
        }
        let claimed_n = arguments
            .get("claimed_n")
            .and_then(Value::as_u64)
            .ok_or("claimed_n is required and must be a non-negative integer")?;
        if claimed_n > 1_000_000 {
            return Err("claimed_n exceeds the 1000000-subject safety bound".into());
        }
        let transport: DeclaredTransport = serde_json::from_value(
            arguments
                .get("transport")
                .cloned()
                .ok_or("transport is required and must be a serialized DeclaredTransport")?,
        )
        .map_err(|error| format!("invalid declared transport: {error}"))?;

        let fidelity_axes: Vec<Value> = result
            .rests_on
            .iter()
            .map(|axis| {
                json!({
                    "axis": axis,
                    "passage": result.model.passage,
                    "measured": fidelity.covers(*axis, result.model.passage)
                })
            })
            .collect();
        let transport_assumption_names: Vec<String> =
            transport.assumption_names().map(str::to_owned).collect();
        let model_identity = json!({
            "model": result.model.model,
            "system": result.model.system,
            "source_specimen": result.model.source_specimen,
            "passage": result.model.passage,
            "verified_against_source": result.model.verified_against_source
        });
        let replicate_summary = json!({
            "technical_wells": result.replicates.technical_wells,
            "biological_replicates": result.replicates.biological_replicates,
            "effective_biological_n": result.replicates.effective_n(),
            "claimed_n": claimed_n
        });
        let establishment_summary = json!({
            "attempted": cohort.attempted,
            "established": cohort.established,
            "selected": cohort.is_selected(),
            "selection_modelled": cohort.selection_modelled
        });

        match onco_transport_model(&result, &fidelity, cohort, claimed_n as usize, &transport) {
            Ok(claim) => Ok(json!({
                "ok": true,
                "schema": "bioprism-mcp/oncoworlds-model-transport/0.1",
                "supported": true,
                "outcome_kind": "supported",
                "model_statement": result.as_stated(),
                "effect": result.effect,
                "model_identity": model_identity,
                "rests_on": result.rests_on,
                "fidelity_axes": fidelity_axes,
                "establishment": establishment_summary,
                "replicates": replicate_summary,
                "transport_assumption_names": transport_assumption_names,
                "required_assumptions": MODEL_REQUIRED_ASSUMPTIONS,
                "effective_biological_n": result.replicates.effective_n(),
                "patient_relevant_claim": claim,
                "guarantees": [
                    "model identity is checked before any patient-level transport",
                    "fidelity is required at the passage on which the effect was observed",
                    "establishment selection and technical-versus-biological replication remain explicit",
                    "the transport carries a loss ledger and every required assumption"
                ],
                "limitations": [
                    "the tool checks declared transport structure; it does not validate identity, fidelity, or assumptions against an external oracle",
                    "a supported patient-relevant claim is still a research transport, not a clinical treatment recommendation"
                ]
            })),
            Err(refusal) => {
                let refusal_value =
                    serde_json::to_value(&refusal).map_err(|error| error.to_string())?;
                Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/oncoworlds-model-transport/0.1",
                    "supported": false,
                    "outcome_kind": "refused",
                    "refusal_kind": refusal_value["refusal"],
                    "stage": "model_to_patient_transport",
                    "refusal": refusal_value,
                    "refusal_text": refusal.to_string(),
                    "fail_closed": true,
                    "model_statement": result.as_stated(),
                    "effect": result.effect,
                    "model_identity": model_identity,
                    "rests_on": result.rests_on,
                    "fidelity_axes": fidelity_axes,
                    "establishment": establishment_summary,
                    "replicates": replicate_summary,
                    "transport_assumption_names": transport_assumption_names,
                    "required_assumptions": MODEL_REQUIRED_ASSUMPTIONS,
                    "guarantee": "a model-system effect is never relabelled as patient evidence when an identity, fidelity, selection, replication, loss, or assumption boundary is missing"
                }))
            }
        }
    }

    pub(super) fn oncoworlds_methylation_classify(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let classifier: ClassifierVersion = serde_json::from_value(
            arguments
                .get("classifier")
                .cloned()
                .ok_or("classifier is required and must be a ClassifierVersion")?,
        )
        .map_err(|error| format!("invalid classifier version: {error}"))?;
        let scores: std::collections::BTreeMap<
            MethylationClass,
            bioprism_oncoworlds::CalibratedScore,
        > = serde_json::from_value(arguments.get("scores").cloned().ok_or(
            "scores is required and must be a map from class labels to calibrated scores",
        )?)
        .map_err(|error| format!("invalid calibrated score map: {error}"))?;
        if scores.len() > 10_000 {
            return Err("methylation score map exceeds the 10000-class safety bound".into());
        }
        let context: SampleContext = serde_json::from_value(
            arguments
                .get("context")
                .cloned()
                .ok_or("context is required and must be a SampleContext")?,
        )
        .map_err(|error| format!("invalid methylation sample context: {error}"))?;
        let classifier_value =
            serde_json::to_value(&classifier).map_err(|error| error.to_string())?;
        let qc_value = serde_json::to_value(&context.qc).map_err(|error| error.to_string())?;
        let tumour_content_value =
            serde_json::to_value(context.tumour_content).map_err(|error| error.to_string())?;
        let score_classes: Vec<String> = scores
            .keys()
            .map(|class| class.as_str().to_owned())
            .collect();
        match onco_classify_methylation(&classifier, &scores, &context) {
            Ok(report) => {
                let classified = report.outcome.is_classified();
                let class = report.outcome.class().cloned();
                let report_value =
                    serde_json::to_value(&report).map_err(|error| error.to_string())?;
                let outcome_kind = report_value["outcome"]["outcome"]
                    .as_str()
                    .ok_or("methylation report must carry a tagged outcome")?;
                Ok(json!({
                    "ok": true,
                    "schema": "bioprism-mcp/oncoworlds-methylation-classify/0.1",
                    "outcome_kind": outcome_kind,
                    "classified": classified,
                    "class": class,
                    "classifier": classifier_value,
                    "classifier_threshold": classifier.reporting_threshold,
                    "threshold_declared": classifier.reporting_threshold.is_some(),
                    "qc": qc_value,
                    "tumour_content": tumour_content_value,
                    "score_count": scores.len(),
                    "score_classes": score_classes,
                    "caveat_count": report.caveats.len(),
                    "nearest_present": report_value["outcome"]["nearest"].is_object(),
                    "report": report,
                    "denominator_policy": "an unclassifiable result remains a result and must stay in the evaluation denominator",
                    "guarantees": [
                        "a classifier must declare its own reporting threshold",
                        "quality-control failure and below-threshold abstention remain distinct outcomes",
                        "nearest class evidence never becomes a class call",
                        "tumour-content absence is retained as a caveat rather than silently imputed"
                    ],
                    "limitations": [
                        "no array preprocessing, batch correction, classifier fitting, or tumour-content threshold is inferred",
                        "the tool evaluates supplied calibrated scores; it does not validate the classifier against a reference cohort"
                    ]
                }))
            }
            Err(refusal) => Ok(json!({
                "ok": false,
                "schema": "bioprism-mcp/oncoworlds-methylation-classify/0.1",
                "outcome_kind": "refused",
                "refusal_kind": serde_json::to_value(&refusal)
                    .map_err(|error| error.to_string())?["refusal"].clone(),
                "stage": "methylation_classification",
                "refusal": serde_json::to_value(&refusal).map_err(|error| error.to_string())?,
                "refusal_text": refusal.to_string(),
                "fail_closed": true,
                "classifier": classifier_value,
                "classifier_threshold": classifier.reporting_threshold,
                "threshold_declared": classifier.reporting_threshold.is_some(),
                "qc": qc_value,
                "tumour_content": tumour_content_value,
                "score_count": scores.len(),
                "score_classes": score_classes,
                "guarantee": "an undeclared threshold or other typed methylation refusal never becomes a maximum-score class call"
            })),
        }
    }

    pub(super) fn oncoworlds_methylation_compare(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let left: VersionedResult = serde_json::from_value(
            arguments
                .get("left")
                .cloned()
                .ok_or("left is required and must be a VersionedResult")?,
        )
        .map_err(|error| format!("invalid left versioned result: {error}"))?;
        let right: VersionedResult = serde_json::from_value(
            arguments
                .get("right")
                .cloned()
                .ok_or("right is required and must be a VersionedResult")?,
        )
        .map_err(|error| format!("invalid right versioned result: {error}"))?;
        let comparison = onco_reconcile_methylation(&left, &right);
        let comparison_value =
            serde_json::to_value(&comparison).map_err(|error| error.to_string())?;
        let left_outcome =
            serde_json::to_value(&left.outcome).map_err(|error| error.to_string())?;
        let right_outcome =
            serde_json::to_value(&right.outcome).map_err(|error| error.to_string())?;
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/oncoworlds-methylation-compare/0.1",
            "divergence_kind": comparison_value["divergence"]["divergence"],
            "classifier_changed": left.classifier != right.classifier,
            "left_outcome_kind": left_outcome["outcome"],
            "right_outcome_kind": right_outcome["outcome"],
            "stable_evidence_count": comparison.stable_evidence.len(),
            "comparison": comparison,
            "left_classifier": left.classifier,
            "right_classifier": right.classifier,
            "guarantees": [
                "classifier-version changes are reported as version-conditioned evidence",
                "the earlier result is not rewritten as wrong merely because a reference updated",
                "two unclassifiable results remain distinct from agreement on a class"
            ],
            "limitations": [
                "no cross-version ontology mapping or stable corroborating evidence is inferred",
                "this compares supplied results and does not rerun either classifier"
            ]
        }))
    }

    pub(super) fn oncoworlds_radiogenomic_check(&self, arguments: &Value) -> Result<Value, String> {
        let claim: RadiogenomicClaim = serde_json::from_value(
            arguments
                .get("claim")
                .cloned()
                .ok_or("claim is required and must be a RadiogenomicClaim")?,
        )
        .map_err(|error| format!("invalid radiogenomic claim: {error}"))?;
        let design: EvaluationDesign = serde_json::from_value(
            arguments
                .get("design")
                .cloned()
                .ok_or("design is required and must be an EvaluationDesign")?,
        )
        .map_err(|error| format!("invalid radiogenomic evaluation design: {error}"))?;
        let observation: bioprism_oncoworlds::SpecimenObservation = serde_json::from_value(
            arguments
                .get("observation")
                .cloned()
                .ok_or("observation is required and must be a SpecimenObservation")?,
        )
        .map_err(|error| format!("invalid specimen observation: {error}"))?;
        let transport: DeclaredTransport = serde_json::from_value(
            arguments
                .get("transport")
                .cloned()
                .ok_or("transport is required and must be a DeclaredTransport")?,
        )
        .map_err(|error| format!("invalid declared transport: {error}"))?;
        let claim_target = serde_json::to_value(claim.target).map_err(|error| error.to_string())?;
        let claim_statement = claim.statement.clone();
        let design_summary = json!({
            "split_unit": design.split_unit,
            "feature_provenance": design.feature_provenance,
            "feature_version": design.feature_version,
            "external_cohort": design.external_cohort,
            "strata": design.strata,
            "mechanism_strata_present": MECHANISM_STRATA.iter().all(|stratum| design.strata.contains(*stratum))
        });
        let transport_assumption_names: Vec<String> =
            transport.assumption_names().map(str::to_owned).collect();
        match onco_assert_radiogenomic_claim(claim, &design, &observation, &transport) {
            Ok(supported) => Ok(json!({
                "ok": true,
                "schema": "bioprism-mcp/oncoworlds-radiogenomic-check/0.1",
                "supported": true,
                "outcome_kind": "supported",
                "claim_target": claim_target,
                "claim_statement": claim_statement,
                "design": design_summary,
                "transport_assumption_names": transport_assumption_names,
                "required_assumptions": REQUIRED_ASSUMPTIONS,
                "supported_claim": supported,
                "guarantees": [
                    "participant-safe splitting, training-only feature fitting, and pre-specified external cohorts are checked before claim scope",
                    "specimen negative calls do not silently become tumour-level labels",
                    "mechanism claims require the declared site and scanner strata",
                    "cross-scope transport carries losses and required assumptions"
                ],
                "limitations": [
                    "no imaging feature extraction, predictive model, AUROC, or causal effect is computed",
                    "the tool checks design and scope declarations supplied by the caller"
                ]
            })),
            Err(refusal) => Ok(json!({
                "ok": false,
                "schema": "bioprism-mcp/oncoworlds-radiogenomic-check/0.1",
                "supported": false,
                "outcome_kind": "refused",
                "claim_target": claim_target,
                "claim_statement": claim_statement,
                "design": design_summary,
                "transport_assumption_names": transport_assumption_names,
                "required_assumptions": REQUIRED_ASSUMPTIONS,
                "refusal_kind": serde_json::to_value(&refusal)
                    .map_err(|error| error.to_string())?["refusal"].clone(),
                "stage": "radiogenomic_claim",
                "refusal": serde_json::to_value(&refusal).map_err(|error| error.to_string())?,
                "refusal_text": refusal.to_string(),
                "fail_closed": true,
                "guarantee": "a leaky evaluation, specimen-scoped label, unstratified mechanism claim, or undeclared transport never becomes a supported mechanistic statement"
            })),
        }
    }

    pub(super) fn oncoworlds_clonal_history_check(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let population: TumourPopulation = serde_json::from_value(
            arguments
                .get("population")
                .cloned()
                .ok_or("population is required and must be a TumourPopulation")?,
        )
        .map_err(|error| format!("invalid tumour population: {error}"))?;
        let candidates: Vec<ClonalHistory> = serde_json::from_value(
            arguments
                .get("candidates")
                .cloned()
                .ok_or("candidates is required and must be an array of ClonalHistory values")?,
        )
        .map_err(|error| format!("invalid clonal history candidates: {error}"))?;
        if candidates.len() > 10_000 {
            return Err(
                "clonal history candidate set exceeds the 10000-history safety bound".into(),
            );
        }
        let result = onco_compatible_histories(&population, candidates);
        let compatible_count = result.compatible.len();
        let rejected_count = result.rejected.len();
        let unique = match result.sole() {
            Ok(history) => json!({ "ok": true, "history": history }),
            Err(refusal) => json!({
                "ok": false,
                "refusal": serde_json::to_value(&refusal).map_err(|error| error.to_string())?,
                "refusal_text": refusal.to_string()
            }),
        };
        let unique_status = if unique.get("ok").and_then(Value::as_bool) == Some(true) {
            "unique"
        } else if unique["refusal"]["refusal"] == json!("ambiguous") {
            "ambiguous"
        } else {
            "refused"
        };
        let rejected_records: Vec<Value> = result
            .rejected
            .iter()
            .map(|(history, refusal)| {
                let refusal_value =
                    serde_json::to_value(refusal).expect("clonal refusal is serializable");
                let refusal_kind = refusal_value
                    .get("refusal")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                json!({
                    "history": history,
                    "refusal": refusal,
                    "refusal_kind": refusal_kind,
                    "refusal_text": refusal.to_string(),
                })
            })
            .collect();
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/oncoworlds-clonal-history-check/0.1",
            "compatible_count": compatible_count,
            "rejected_count": rejected_count,
            "candidate_count": compatible_count + rejected_count,
            "compatible": result.compatible,
            "rejected": result.rejected,
            "rejected_records": rejected_records,
            "unique_history": unique,
            "unique_status": unique_status,
            "guarantees": [
                "candidate histories are audited against fraction, ancestry, cycle, and whole-tumour constraints",
                "rejected histories remain visible with their typed refusal",
                "multiple compatible histories remain ambiguity rather than being collapsed to the first candidate",
                "the tool checks supplied histories and does not infer a phylogeny from raw variant data"
            ],
            "limitations": [
                "no clonal tree enumeration, sequencing error model, or causal treatment attribution is performed",
                "compatibility is arithmetic consistency, not proof that one history is biologically true"
            ]
        }))
    }

    pub(super) fn oncoworlds_clonal_evidence_check(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let mut checks = serde_json::Map::new();

        if let Some(value) = arguments.get("promotion") {
            let object = value.as_object().ok_or("promotion must be an object")?;
            let observation: SpecimenObservation = serde_json::from_value(
                object
                    .get("observation")
                    .cloned()
                    .ok_or("promotion.observation is required")?,
            )
            .map_err(|error| format!("invalid promotion specimen observation: {error}"))?;
            let observation_value =
                serde_json::to_value(&observation).map_err(|error| error.to_string())?;
            let marker_value =
                serde_json::to_value(observation.marker).map_err(|error| error.to_string())?;
            let mut projection = json!({
                "observation": observation_value,
                "marker": marker_value,
                "allowed": false
            });
            match observation.as_tumour_claim() {
                Ok(claim) => {
                    let claim_value =
                        serde_json::to_value(&claim).map_err(|error| error.to_string())?;
                    projection["allowed"] = json!(true);
                    projection["outcome_kind"] = claim_value
                        .get("tumour_claim")
                        .cloned()
                        .unwrap_or(Value::Null);
                    projection["tumour_claim"] = claim_value;
                    projection["refusal"] = Value::Null;
                    projection["refusal_kind"] = Value::Null;
                }
                Err(refusal) => {
                    let refusal_value =
                        serde_json::to_value(&refusal).map_err(|error| error.to_string())?;
                    projection["outcome_kind"] = json!("refused");
                    projection["refusal"] = refusal_value.clone();
                    projection["refusal_kind"] = refusal_value["refusal"].clone();
                    projection["refusal_text"] = json!(refusal.to_string());
                }
            }
            checks.insert("promotion".into(), projection);
        }

        if let Some(value) = arguments.get("resistance") {
            let object = value.as_object().ok_or("resistance must be an object")?;
            let diagnosis: SpecimenObservation = serde_json::from_value(
                object
                    .get("diagnosis")
                    .cloned()
                    .ok_or("resistance.diagnosis is required")?,
            )
            .map_err(|error| format!("invalid resistance diagnosis observation: {error}"))?;
            let recurrence: SpecimenObservation = serde_json::from_value(
                object
                    .get("recurrence")
                    .cloned()
                    .ok_or("resistance.recurrence is required")?,
            )
            .map_err(|error| format!("invalid resistance recurrence observation: {error}"))?;
            let explanations = onco_explain_new_alteration(&diagnosis, &recurrence);
            let sole = explanations.sole();
            let mut projection = json!({
                "diagnosis": serde_json::to_value(&diagnosis).map_err(|error| error.to_string())?,
                "recurrence": serde_json::to_value(&recurrence).map_err(|error| error.to_string())?,
                "not_excluded": serde_json::to_value(&explanations.not_excluded).map_err(|error| error.to_string())?,
                "excluded": explanations.excluded,
                "de_novo_emergence_survives": explanations.contains(
                    bioprism_oncoworlds::ResistanceExplanation::DeNovoEmergence
                ),
                "allowed": sole.is_ok(),
                "outcome_kind": if sole.is_ok() { "unique" } else { "ambiguous" }
            });
            match sole {
                Ok(explanation) => {
                    projection["unique_explanation"] =
                        serde_json::to_value(explanation).map_err(|error| error.to_string())?;
                    projection["refusal"] = Value::Null;
                    projection["refusal_kind"] = Value::Null;
                }
                Err(refusal) => {
                    let refusal_value =
                        serde_json::to_value(&refusal).map_err(|error| error.to_string())?;
                    projection["refusal"] = refusal_value.clone();
                    projection["refusal_kind"] = refusal_value["refusal"].clone();
                    projection["refusal_text"] = json!(refusal.to_string());
                }
            }
            checks.insert("resistance".into(), projection);
        }

        if let Some(value) = arguments.get("attribution") {
            let object = value.as_object().ok_or("attribution must be an object")?;
            let treatment = object
                .get("treatment")
                .and_then(Value::as_str)
                .ok_or("attribution.treatment must be a string")?;
            if treatment.len() > 10_000 {
                return Err("attribution treatment exceeds the 10000-byte safety bound".into());
            }
            let alteration: MolecularMarker = serde_json::from_value(
                object
                    .get("alteration")
                    .cloned()
                    .ok_or("attribution.alteration is required")?,
            )
            .map_err(|error| format!("invalid treatment-attribution alteration: {error}"))?;
            let design: CausalDesign = serde_json::from_value(
                object
                    .get("design")
                    .cloned()
                    .ok_or("attribution.design is required")?,
            )
            .map_err(|error| format!("invalid treatment-attribution design: {error}"))?;
            let result = onco_attribute_to_treatment(treatment, alteration, design);
            let mut projection = json!({
                "treatment": treatment,
                "alteration": alteration,
                "alteration_label": alteration.describe(),
                "design": design,
                "allowed": result.is_ok()
            });
            let refusal = result.expect_err("temporal-attribution design is always refused");
            let refusal_value =
                serde_json::to_value(&refusal).map_err(|error| error.to_string())?;
            projection["refusal"] = refusal_value.clone();
            projection["refusal_kind"] = refusal_value["refusal"].clone();
            projection["refusal_text"] = json!(refusal.to_string());
            checks.insert("attribution".into(), projection);
        }

        if checks.is_empty() {
            return Err("at least one clonal evidence section is required".into());
        }
        if checks.len() > 3 {
            return Err("clonal evidence checks may contain at most three sections".into());
        }
        let refusal_count = checks
            .values()
            .filter(|value| value.get("allowed") == Some(&json!(false)))
            .count();
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/oncoworlds-clonal-evidence-check/0.1",
            "outcome_kind": "report",
            "all_admissible": refusal_count == 0,
            "check_count": checks.len(),
            "refusal_count": refusal_count,
            "checks": checks,
            "guarantees": [
                "specimen presence promotes only to a claim over sampled tumour material",
                "negative and below-detection observations retain their assay limit and sampled regions",
                "resistance explanations remain set-valued and recurrence selection is not rewritten as de novo biology",
                "temporal association is never upgraded to treatment causation"
            ],
            "limitations": [
                "no phylogeny is inferred, no allele-fraction conversion is performed, and no causal effect is estimated",
                "a unique explanation would still be an arithmetic survivor, not proof of biological history",
                "these checks do not diagnose, classify, or recommend treatment"
            ]
        }))
    }

    pub(super) fn oncoworlds_era_shift_check(&self, arguments: &Value) -> Result<Value, String> {
        let left: OncoShiftCohort = serde_json::from_value(
            arguments
                .get("left")
                .cloned()
                .ok_or("left is required and must be a serialized OncoWorlds Cohort")?,
        )
        .map_err(|error| format!("invalid left OncoWorlds cohort: {error}"))?;
        let right: OncoShiftCohort = serde_json::from_value(
            arguments
                .get("right")
                .cloned()
                .ok_or("right is required and must be a serialized OncoWorlds Cohort")?,
        )
        .map_err(|error| format!("invalid right OncoWorlds cohort: {error}"))?;
        if left.entities.len() > 10_000 || right.entities.len() > 10_000 {
            return Err("OncoWorlds cohort entity sets exceed the 10000-label safety bound".into());
        }
        let mapping: Option<EntityMapping> = arguments
            .get("mapping")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| format!("invalid OncoWorlds entity mapping: {error}"))?;
        if mapping
            .as_ref()
            .is_some_and(|item| item.fate_count() > 10_000)
        {
            return Err("OncoWorlds entity mapping exceeds the 10000-label safety bound".into());
        }
        let assays: Vec<SiteAssayContext> = arguments
            .get("assay_contexts")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| format!("invalid site assay contexts: {error}"))?
            .unwrap_or_default();
        if assays.len() > 100 {
            return Err("site assay context panel exceeds the 100-context safety bound".into());
        }
        let assay_projections: Vec<Value> = assays
            .iter()
            .map(|context| {
                let availability = serde_json::to_value(&context.availability)
                    .map_err(|error| error.to_string())?;
                let observation = serde_json::to_value(context.observation())
                    .map_err(|error| error.to_string())?;
                let refusal =
                    onco_as_negative_call(context).expect_err("negative conversion always refuses");
                let refusal_value =
                    serde_json::to_value(&refusal).map_err(|error| error.to_string())?;
                Ok(json!({
                    "site": context.site,
                    "assay": context.assay,
                    "availability": availability,
                    "observation": observation,
                    "negative_call_supported": false,
                    "negative_call_refusal": refusal_value,
                    "negative_call_refusal_kind": refusal_value["refusal"]
                }))
            })
            .collect::<Result<_, String>>()?;
        let descriptor_values = arguments
            .get("descriptor_checks")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if descriptor_values.len() > 100 {
            return Err(
                "population descriptor check panel exceeds the 100-check safety bound".into(),
            );
        }
        let descriptor_projections: Vec<Value> = descriptor_values
            .iter()
            .map(|value| {
                let descriptor: PopulationDescriptor = serde_json::from_value(
                    value
                        .get("descriptor")
                        .cloned()
                        .ok_or("descriptor check requires descriptor")?,
                )
                .map_err(|error| format!("invalid population descriptor: {error}"))?;
                let use_: DescriptorUse = serde_json::from_value(
                    value
                        .get("use")
                        .cloned()
                        .ok_or("descriptor check requires use")?,
                )
                .map_err(|error| format!("invalid population descriptor use: {error}"))?;
                let descriptor_value =
                    serde_json::to_value(descriptor).map_err(|error| error.to_string())?;
                let use_value = serde_json::to_value(use_).map_err(|error| error.to_string())?;
                match onco_use_descriptor(descriptor, use_) {
                    Ok(()) => Ok(json!({
                        "descriptor": descriptor_value,
                        "descriptor_label": descriptor.as_str(),
                        "use": use_value,
                        "use_label": use_.as_str(),
                        "administrative": descriptor.is_administrative(),
                        "allowed": true
                    })),
                    Err(refusal) => {
                        let refusal_value =
                            serde_json::to_value(&refusal).map_err(|error| error.to_string())?;
                        Ok(json!({
                            "descriptor": descriptor_value,
                            "descriptor_label": descriptor.as_str(),
                            "use": use_value,
                            "use_label": use_.as_str(),
                            "administrative": descriptor.is_administrative(),
                            "allowed": false,
                            "refusal": refusal_value,
                            "refusal_kind": refusal_value["refusal"],
                            "refusal_text": refusal.to_string()
                        }))
                    }
                }
            })
            .collect::<Result<_, String>>()?;
        let left_value = serde_json::to_value(&left).map_err(|error| error.to_string())?;
        let right_value = serde_json::to_value(&right).map_err(|error| error.to_string())?;
        let mapping_value = mapping
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|error| error.to_string())?;
        let same_classification_version =
            left.classification_version == right.classification_version;
        let mapping_versions_match = mapping.as_ref().is_some_and(|item| {
            (item.from == left.classification_version && item.to == right.classification_version)
                || (item.from == right.classification_version
                    && item.to == left.classification_version)
        });
        let shared = json!({
            "left": left_value,
            "right": right_value,
            "mapping": mapping_value,
            "mapping_declared": mapping.is_some(),
            "mapping_fate_count": mapping.as_ref().map(EntityMapping::fate_count).unwrap_or(0),
            "mapping_versions_match": mapping_versions_match,
            "same_classification_version": same_classification_version,
            "left_entity_count": left.entities.len(),
            "right_entity_count": right.entities.len(),
            "assay_contexts": assay_projections,
            "assay_context_count": assays.len(),
            "descriptor_checks": descriptor_projections,
            "descriptor_check_count": descriptor_values.len()
        });
        match onco_comparable_cohorts(&left, &right, mapping.as_ref()) {
            Ok(()) => Ok(json!({
                "ok": true,
                "schema": "bioprism-mcp/oncoworlds-era-shift-check/0.1",
                "outcome_kind": "comparable",
                "comparable": true,
                "evidence": shared,
                "guarantees": [
                    "same-version cohorts do not require an invented mapping",
                    "cross-version comparison requires a stated mapping covering every label present in the older cohort",
                    "resource absence remains not-collected and never becomes a negative molecular call",
                    "administrative population descriptors may stratify a report but do not become biological mechanisms"
                ],
                "limitations": [
                    "label mappings are caller-declared and no classification semantics are inferred",
                    "assay and descriptor checks are evidence projections; no assay is run and no biological effect is estimated"
                ]
            })),
            Err(refusal) => {
                let refusal_value =
                    serde_json::to_value(&refusal).map_err(|error| error.to_string())?;
                Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/oncoworlds-era-shift-check/0.1",
                    "outcome_kind": "refused",
                    "stage": "classification_era_comparability",
                    "comparable": false,
                    "refusal_kind": refusal_value["refusal"],
                    "refusal": refusal_value,
                    "refusal_text": refusal.to_string(),
                    "fail_closed": true,
                    "evidence": shared,
                    "guarantee": "an unmapped era change, incomplete label mapping, or unsupported shift descriptor never becomes a comparable cohort claim"
                }))
            }
        }
    }

    pub(super) fn oncoworlds_equity_check(&self, arguments: &Value) -> Result<Value, String> {
        let pooled: PooledScore = serde_json::from_value(
            arguments
                .get("pooled")
                .cloned()
                .ok_or("pooled is required and must be a serialized PooledScore")?,
        )
        .map_err(|error| format!("invalid pooled score: {error}"))?;
        if pooled.subgroups.len() > 10_000 {
            return Err("equity subgroup panel exceeds the 10000-subgroup safety bound".into());
        }
        let pooled_value = pooled.value;
        let subgroup_count = pooled.subgroups.len();
        let interval_count = pooled
            .subgroups
            .iter()
            .filter(|subgroup| subgroup.interval.is_some())
            .count();
        let subgroup_values: Vec<Value> = pooled
            .subgroups
            .iter()
            .map(serde_json::to_value)
            .collect::<Result<_, _>>()
            .map_err(|error| error.to_string())?;
        match onco_equity_report(pooled) {
            Ok(report) => {
                let claims: Vec<Value> = report
                    .subgroups()
                    .iter()
                    .map(|claim| serde_json::to_value(claim.result()))
                    .collect::<Result<_, _>>()
                    .map_err(|error| error.to_string())?;
                Ok(json!({
                    "ok": true,
                    "schema": "bioprism-mcp/oncoworlds-equity-check/0.1",
                    "outcome_kind": "equity_report",
                    "equity_supported": true,
                    "pooled_value": report.pooled(),
                    "subgroups": claims,
                    "subgroup_count": subgroup_count,
                    "interval_count": interval_count,
                    "all_intervals_present": interval_count == subgroup_count,
                    "report": report,
                    "guarantees": [
                        "a pooled score is never released as an equity claim without every subgroup result",
                        "each published subgroup carries its own sample size and uncertainty interval",
                        "small subgroups are not silently discarded; their instability remains visible in the interval"
                    ],
                    "limitations": [
                        "the endpoint does not compute estimates, intervals, calibration, or fairness metrics",
                        "an interval is evidence of uncertainty, not proof of parity or absence of bias"
                    ]
                }))
            }
            Err(refusal) => {
                let refusal_value =
                    serde_json::to_value(&refusal).map_err(|error| error.to_string())?;
                Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/oncoworlds-equity-check/0.1",
                    "outcome_kind": "refused",
                    "stage": "equity_report",
                    "equity_supported": false,
                    "refusal_kind": refusal_value["refusal"],
                    "refusal": refusal_value,
                    "refusal_text": refusal.to_string(),
                    "fail_closed": true,
                    "pooled_value": pooled_value,
                    "subgroups": subgroup_values,
                    "subgroup_count": subgroup_count,
                    "interval_count": interval_count,
                    "all_intervals_present": interval_count == subgroup_count,
                    "guarantee": "a pooled-only, empty, or unquantified subgroup result never becomes an equity claim"
                }))
            }
        }
    }

    pub(super) fn oncoworlds_entity_world_check(&self, arguments: &Value) -> Result<Value, String> {
        let mut checks = serde_json::Map::new();

        if let Some(value) = arguments.get("provenance") {
            let object = value.as_object().ok_or("provenance must be an object")?;
            let left: TissueProvenance = serde_json::from_value(
                object
                    .get("left")
                    .cloned()
                    .ok_or("provenance.left is required")?,
            )
            .map_err(|error| format!("invalid left tissue provenance: {error}"))?;
            let right: TissueProvenance = serde_json::from_value(
                object
                    .get("right")
                    .cloned()
                    .ok_or("provenance.right is required")?,
            )
            .map_err(|error| format!("invalid right tissue provenance: {error}"))?;
            let selection_modelled = object
                .get("selection_modelled")
                .and_then(Value::as_bool)
                .ok_or("provenance.selection_modelled must be a boolean")?;
            let result = onco_pool_provenance(left, right, selection_modelled);
            let mut projection = json!({
                "left": left,
                "right": right,
                "left_label": left.as_str(),
                "right_label": right.as_str(),
                "selection_modelled": selection_modelled,
                "allowed": result.is_ok()
            });
            if let Err(refusal) = result {
                let refusal_value =
                    serde_json::to_value(&refusal).map_err(|error| error.to_string())?;
                projection["refusal"] = refusal_value.clone();
                projection["refusal_kind"] = refusal_value["refusal"].clone();
                projection["refusal_text"] = json!(refusal.to_string());
            } else {
                projection["refusal"] = Value::Null;
                projection["refusal_kind"] = Value::Null;
            }
            checks.insert("provenance".into(), projection);
        }

        if let Some(value) = arguments.get("alterations") {
            let object = value.as_object().ok_or("alterations must be an object")?;
            let left: AlterationMechanism = serde_json::from_value(
                object
                    .get("left")
                    .cloned()
                    .ok_or("alterations.left is required")?,
            )
            .map_err(|error| format!("invalid left alteration mechanism: {error}"))?;
            let right: AlterationMechanism = serde_json::from_value(
                object
                    .get("right")
                    .cloned()
                    .ok_or("alterations.right is required")?,
            )
            .map_err(|error| format!("invalid right alteration mechanism: {error}"))?;
            let estimand = object.get("estimand").and_then(Value::as_str);
            let result = onco_pool_alterations(left, right, estimand);
            let mut projection = json!({
                "left": left,
                "right": right,
                "left_label": left.as_str(),
                "right_label": right.as_str(),
                "estimand": estimand,
                "estimand_declared": estimand.is_some_and(|text| !text.trim().is_empty()),
                "allowed": result.is_ok()
            });
            if let Err(refusal) = result {
                let refusal_value =
                    serde_json::to_value(&refusal).map_err(|error| error.to_string())?;
                projection["refusal"] = refusal_value.clone();
                projection["refusal_kind"] = refusal_value["refusal"].clone();
                projection["refusal_text"] = json!(refusal.to_string());
            } else {
                projection["refusal"] = Value::Null;
                projection["refusal_kind"] = Value::Null;
            }
            checks.insert("alterations".into(), projection);
        }

        if let Some(value) = arguments.get("benchmark") {
            let object = value.as_object().ok_or("benchmark must be an object")?;
            let macro_score = object
                .get("macro_score")
                .and_then(Value::as_f64)
                .ok_or("benchmark.macro_score must be a number")?;
            let counts: std::collections::BTreeMap<String, usize> = serde_json::from_value(
                object
                    .get("per_class_counts")
                    .cloned()
                    .ok_or("benchmark.per_class_counts is required")?,
            )
            .map_err(|error| format!("invalid per-class counts: {error}"))?;
            if counts.len() > 10_000 {
                return Err("benchmark class counts exceed the 10000-class safety bound".into());
            }
            let report = RarePerformanceReport {
                macro_score,
                per_class_counts: counts.clone(),
            };
            let report_value = serde_json::to_value(&report).map_err(|error| error.to_string())?;
            let mut projection = json!({
                "macro_score": macro_score,
                "per_class_counts": counts,
                "class_count": report.per_class_counts.len(),
                "zero_case_classes": report.per_class_counts.iter().filter_map(|(class, count)| (*count == 0).then_some(class)).collect::<Vec<_>>(),
                "report": report_value,
                "allowed": false,
                "published": Value::Null
            });
            match report.publish() {
                Ok(published) => {
                    let feasibility = serde_json::to_value(published.feasibility())
                        .map_err(|error| error.to_string())?;
                    projection["allowed"] = json!(true);
                    projection["published"] =
                        serde_json::to_value(&published).map_err(|error| error.to_string())?;
                    projection["feasibility"] = feasibility.clone();
                    projection["feasibility_kind"] = feasibility["feasibility"].clone();
                    projection["refusal"] = Value::Null;
                    projection["refusal_kind"] = Value::Null;
                }
                Err(refusal) => {
                    let refusal_value =
                        serde_json::to_value(&refusal).map_err(|error| error.to_string())?;
                    projection["refusal"] = refusal_value.clone();
                    projection["refusal_kind"] = refusal_value["refusal"].clone();
                    projection["refusal_text"] = json!(refusal.to_string());
                }
            }
            checks.insert("benchmark".into(), projection);
        }

        if let Some(value) = arguments.get("lesion_analysis") {
            let object = value
                .as_object()
                .ok_or("lesion_analysis must be an object")?;
            let set = LesionSet {
                lesions: object
                    .get("lesions")
                    .and_then(Value::as_u64)
                    .ok_or("lesion_analysis.lesions must be a non-negative integer")?
                    as usize,
                participants: object
                    .get("participants")
                    .and_then(Value::as_u64)
                    .ok_or("lesion_analysis.participants must be a non-negative integer")?
                    as usize,
            };
            if set.lesions > 1_000_000 || set.participants > 1_000_000 {
                return Err(
                    "lesion analysis counts exceed the 1000000-subject safety bound".into(),
                );
            }
            let cluster_declared = object
                .get("cluster_declared")
                .and_then(Value::as_bool)
                .ok_or("lesion_analysis.cluster_declared must be a boolean")?;
            let endpoint: LesionEndpoint = serde_json::from_value(
                object
                    .get("endpoint")
                    .cloned()
                    .ok_or("lesion_analysis.endpoint is required")?,
            )
            .map_err(|error| format!("invalid lesion endpoint: {error}"))?;
            let event: FollowUpEvent = serde_json::from_value(
                object
                    .get("event")
                    .cloned()
                    .ok_or("lesion_analysis.event is required")?,
            )
            .map_err(|error| format!("invalid follow-up event: {error}"))?;
            let handling: EventHandling = serde_json::from_value(
                object
                    .get("handling")
                    .cloned()
                    .ok_or("lesion_analysis.handling is required")?,
            )
            .map_err(|error| format!("invalid event handling: {error}"))?;
            let cluster_result = onco_declare_cluster(set, cluster_declared);
            let event_result = onco_handle_event(endpoint, event, handling);
            let mut projection = json!({
                "lesions": set.lesions,
                "participants": set.participants,
                "cluster_declared": cluster_declared,
                "cluster_allowed": cluster_result.is_ok(),
                "endpoint": endpoint,
                "event": event,
                "handling": handling,
                "event_allowed": event_result.is_ok(),
                "allowed": cluster_result.is_ok() && event_result.is_ok()
            });
            if let Err(refusal) = cluster_result {
                let refusal_value =
                    serde_json::to_value(&refusal).map_err(|error| error.to_string())?;
                projection["cluster_refusal"] = refusal_value.clone();
                projection["cluster_refusal_kind"] = refusal_value["refusal"].clone();
                projection["cluster_refusal_text"] = json!(refusal.to_string());
            } else {
                projection["cluster_refusal"] = Value::Null;
                projection["cluster_refusal_kind"] = Value::Null;
            }
            if let Err(refusal) = event_result {
                let refusal_value =
                    serde_json::to_value(&refusal).map_err(|error| error.to_string())?;
                projection["event_refusal"] = refusal_value.clone();
                projection["event_refusal_kind"] = refusal_value["refusal"].clone();
                projection["event_refusal_text"] = json!(refusal.to_string());
            } else {
                projection["event_refusal"] = Value::Null;
                projection["event_refusal_kind"] = Value::Null;
            }
            checks.insert("lesion_analysis".into(), projection);
        }

        if checks.is_empty() {
            return Err("at least one entity-world check section is required".into());
        }
        if checks.len() > 4 {
            return Err("entity-world checks may contain at most four sections".into());
        }
        let refusal_count = checks
            .values()
            .filter(|value| value.get("allowed") == Some(&json!(false)))
            .count();
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/oncoworlds-entity-world-check/0.1",
            "outcome_kind": "report",
            "all_admissible": refusal_count == 0,
            "check_count": checks.len(),
            "refusal_count": refusal_count,
            "checks": checks,
            "guarantees": [
                "provenance selection is explicit before diagnostic, recurrence, and postmortem material is pooled",
                "different alteration mechanisms require a stated estimand rather than pathway-level collapse",
                "rare-class macro performance retains per-class counts and zero-case feasibility findings",
                "lesion analyses declare participant clustering and systemic death is not silently censored for local control"
            ],
            "limitations": [
                "the tool checks declared evidence and does not estimate effect sizes, survival, competing-risk curves, or fairness metrics",
                "a permitted section is a structural admissibility result, not biological truth or clinical authorization"
            ]
        }))
    }

    pub(super) fn oncoworlds_federated_statistical_analysis_workbench(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an OncoworldsAnalysisWorkbenchRequest")?;
        let receipt =
            crate::research_contracts::operate_oncoworlds_analysis_workbench_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_oncoworlds::ONCOWORLDS_ANALYSIS_WORKBENCH_FEATURE_ID,
            "contract_version": bioprism_oncoworlds::ONCOWORLDS_ANALYSIS_WORKBENCH_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "A1 federated continual OncoWorlds analysis attestations are deterministically ordered across study, modality, and model axes",
                "qualified, approval-required, unresolved, blocked, and missing candidate states preserve replay, provenance, evidence, omission, uncertainty, and negative-result witnesses",
                "the researcher workbench is read-only, keeps raw preclinical data institution-local, and emits only a digest-bound view effect"
            ],
            "limitations": [
                "the workbench evaluates caller-supplied analysis attestations and does not fit models, retrieve data, or infer biological truth",
                "qualified output is a bounded researcher view rather than a completed computation, publication decision, or clinical conclusion"
            ]
        }))
    }

    pub(super) fn oncoworlds_prospective_evidence_surveillance_copilot(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or(
            "request is required and must be an OncoworldsEvidenceSurveillanceCopilotRequest",
        )?;
        let receipt =
            crate::research_contracts::operate_oncoworlds_evidence_surveillance_copilot_json(
                request,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_oncoworlds::ONCOWORLDS_EVIDENCE_SURVEILLANCE_COPILOT_FEATURE_ID,
            "contract_version": bioprism_oncoworlds::ONCOWORLDS_EVIDENCE_SURVEILLANCE_COPILOT_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "A2 prospective high-throughput evidence surveillance ranks typed OncoWorlds observations by deterministic checkpointed order",
                "availability, relevance, digest, evidence state, policy, protected closure, approval, locality, capacity, overflow, omission, uncertainty, and negative-result states remain explicit",
                "only declared bounded-tool effects are emitted after approval; raw preclinical evidence remains local and unsafe postures fail closed"
            ],
            "limitations": [
                "the copilot evaluates caller-supplied feed observations and does not retrieve sources, authenticate providers, or interpret biological truth",
                "qualified output is an evidence-alert artifact, not completed synthesis, publication acceptance, or a clinical decision"
            ]
        }))
    }

    pub(super) fn oncoworlds_prospective_replication_negative_results_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an OncoworldsClaimAndProtocol")?;
        let receipt =
            crate::research_contracts::operate_oncoworlds_replication_assurance_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_oncoworlds::ONCOWORLDS_REPLICATION_ASSURANCE_FEATURE_ID,
            "contract_version": bioprism_oncoworlds::ONCOWORLDS_REPLICATION_ASSURANCE_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "federated continual replication claims are deterministically partitioned into admitted, unresolved, blocked, reproduced, and negative-result evidence",
                "null, failed, contradictory, omitted, uncertain, and adversarial outcomes remain explicit and never become a confident conclusion",
                "raw preclinical data stays institution-local; replay, provenance, policy, approval, federation, and aggregate-only gates fail closed"
            ],
            "limitations": [
                "the assurance harness evaluates caller-supplied typed claims and does not run protocols, fetch raw measurements, or decide biological truth",
                "a qualified record is release evidence for research replication, not clinical advice or a substitute for independent scientific review"
            ]
        }))
    }

    pub(super) fn oncoworlds_federated_resource_discovery_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an OncoworldsResourceNeed4")?;
        let receipt =
            crate::research_contracts::operate_oncoworlds_resource_discovery_assurance_json(
                &json!({
                    "request": request,
                    "endpoints": arguments.get("endpoints").ok_or("endpoints are required")?,
                    "peers": arguments.get("peers").ok_or("peers are required")?
                }),
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_oncoworlds::ONCOWORLDS_RESOURCE_DISCOVERY_FEATURE_ID,
            "contract_version": bioprism_oncoworlds::ONCOWORLDS_RESOURCE_DISCOVERY_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "federated continual resource candidates are deterministically fitness-ranked and partitioned into qualified, unresolved, blocked, and missing-capability states",
                "stale, protected, revoked, unavailable, contradictory, unknown, unmeasured, omitted, negative, migration, quorum, and adversarial evidence remains explicit",
                "raw preclinical data remains institution-local; the A1 route emits only verification or unsafe-release block receipts and never fetches endpoints"
            ],
            "limitations": [
                "the assurance harness evaluates caller-supplied endpoint and peer manifests and does not connect to resources or verify biological validity",
                "qualified output is a policy-bound registry receipt, not a protocol execution, publication decision, or clinical decision"
            ]
        }))
    }
}
