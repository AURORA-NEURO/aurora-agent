//! MCP Modality support and comparability handlers.
//!
//! Kept beside the dispatcher so each domain surface can be reviewed independently.

use super::*;

impl Server {
    pub(super) fn modality_catalog(&self, arguments: &Value) -> Result<Value, String> {
        let include_failure_modes = arguments
            .get("include_failure_modes")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let requested = arguments.get("modality").and_then(Value::as_str);
        let descriptors = all_modalities();
        let selected = descriptors
            .iter()
            .filter(|descriptor| requested.is_none_or(|name| descriptor.modality.as_str() == name))
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(format!(
                "unknown modality {:?}; call modality_catalog without a filter to list all modalities",
                requested.unwrap_or("")
            ));
        }

        let mut mechanised = 0usize;
        let mut unmechanised = 0usize;
        let modalities = selected
            .into_iter()
            .map(|descriptor| {
                for mode in descriptor.failure_modes() {
                    if mode.is_mechanised() {
                        mechanised += 1;
                    } else {
                        unmechanised += 1;
                    }
                }
                let mut item = json!({
                    "modality": descriptor.modality.as_str(),
                    "blueprint_module": descriptor.modality.blueprint_module(),
                    "measurand": descriptor.measurand.as_str(),
                    "purpose": descriptor.purpose,
                    "design": descriptor.design.as_str(),
                    "complete": descriptor.is_complete(),
                    "resolutions": Resolution::ALL.into_iter().map(|axis| json!({
                        "axis": axis,
                        "status": descriptor.resolution(axis),
                    })).collect::<Vec<_>>(),
                    "caller_supplied_constants": descriptor.caller_supplied_constants(),
                    "failure_mode_count": descriptor.failure_modes().len(),
                });
                if include_failure_modes {
                    item["failure_modes"] = json!(descriptor.failure_modes());
                }
                item
            })
            .collect::<Vec<_>>();

        Ok(json!({
            "ok": true,
            "returned": modalities.len(),
            "total_catalogue": Modality::ALL.len(),
            "mechanised_failure_modes": mechanised,
            "unmechanised_failure_modes": unmechanised,
            "failure_modes_are_not_claims_of_detection": true,
            "modalities": modalities,
        }))
    }

    pub(super) fn modality_support_check(&self, arguments: &Value) -> Result<Value, String> {
        let modality: Modality = serde_json::from_value(
            arguments
                .get("modality")
                .cloned()
                .ok_or("modality is required and must be a serialized Modality")?,
        )
        .map_err(|error| format!("invalid modality: {error}"))?;
        let claim: ClaimKind = serde_json::from_value(
            arguments
                .get("claim")
                .cloned()
                .ok_or("claim is required and must be a serialized ClaimKind")?,
        )
        .map_err(|error| format!("invalid modality claim kind: {error}"))?;
        let descriptor: ModalityDescriptor = match arguments.get("descriptor") {
            None | Some(Value::Null) => bioprism_modalities::catalog::descriptor(modality),
            Some(value) => serde_json::from_value(value.clone())
                .map_err(|error| format!("invalid modality descriptor: {error}"))?,
        };
        if descriptor.modality != modality {
            return Err(format!(
                "descriptor modality {:?} does not match requested modality {:?}",
                descriptor.modality, modality
            ));
        }
        if descriptor.purpose.len() > 100_000 {
            return Err("modality descriptor purpose exceeds the 100000-byte safety bound".into());
        }
        if descriptor.failure_modes().len() > 100 {
            return Err("modality descriptor exceeds the 100-failure-mode safety bound".into());
        }
        if descriptor.caller_supplied_constants().len() > 100 {
            return Err("modality descriptor exceeds the 100 caller-constant safety bound".into());
        }
        let counted_unit: Option<Resolution> = arguments
            .get("counted_unit")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| format!("invalid counted analysis unit: {error}"))?;
        let support = modality_supports_descriptor(&descriptor, claim);
        let support_projection = match &support {
            Ok(()) => json!({
                "supported": true,
                "refusal": Value::Null,
                "refusal_kind": Value::Null,
                "root_refusal_kind": Value::Null,
                "refusal_text": Value::Null,
            }),
            Err(refusal) => {
                let refusal_value = serde_json::to_value(refusal)
                    .map_err(|error| format!("cannot serialize modality refusal: {error}"))?;
                let root_value = serde_json::to_value(refusal.root())
                    .map_err(|error| format!("cannot serialize root modality refusal: {error}"))?;
                json!({
                    "supported": false,
                    "refusal": refusal_value.clone(),
                    "refusal_kind": refusal_value.get("unsupported").cloned(),
                    "root_refusal_kind": root_value.get("unsupported").cloned(),
                    "refusal_text": refusal.to_string(),
                })
            }
        };
        let analysis_projection = match counted_unit {
            None => json!({
                "requested": false,
                "counted": Value::Null,
                "independent": modality_independent_unit(&descriptor),
                "admissible": Value::Null,
                "refusal": Value::Null,
                "refusal_kind": Value::Null,
                "refusal_text": Value::Null,
            }),
            Some(counted) => match modality_analysis_unit(&descriptor, counted) {
                Ok(()) => json!({
                    "requested": true,
                    "counted": counted,
                    "independent": modality_independent_unit(&descriptor),
                    "admissible": true,
                    "refusal": Value::Null,
                    "refusal_kind": Value::Null,
                    "refusal_text": Value::Null,
                }),
                Err(refusal) => {
                    let refusal_value = serde_json::to_value(&refusal).map_err(|error| {
                        format!("cannot serialize analysis-unit refusal: {error}")
                    })?;
                    json!({
                        "requested": true,
                        "counted": counted,
                        "independent": modality_independent_unit(&descriptor),
                        "admissible": false,
                        "refusal": refusal_value.clone(),
                        "refusal_kind": refusal_value.get("unsupported").cloned(),
                        "refusal_text": refusal.to_string(),
                    })
                }
            },
        };
        let requirements = claim.requirements();
        let resolutions = Resolution::ALL
            .into_iter()
            .map(|axis| {
                json!({
                    "axis": axis,
                    "status": descriptor.resolution(axis),
                })
            })
            .collect::<Vec<_>>();
        let supported_claims = modality_supported_claims(modality)
            .into_iter()
            .map(|supported| supported.as_str())
            .collect::<Vec<_>>();
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/modality-support-check/0.1",
            "outcome_kind": if support.is_ok() { "supported" } else { "refused" },
            "modality": modality,
            "claim": claim,
            "supported": support.is_ok(),
            "claim_requirements": requirements,
            "support": support_projection,
            "analysis_unit": analysis_projection,
            "descriptor": {
                "modality": descriptor.modality,
                "measurand": descriptor.measurand,
                "design": descriptor.design,
                "purpose": descriptor.purpose,
                "complete": descriptor.is_complete(),
                "resolutions": resolutions,
                "resolved_axes": descriptor.resolved_axes(),
                "unresolved_axes": descriptor.unresolved_axes(),
                "undeclared_axes": descriptor.undeclared_axes(),
                "caller_supplied_constants": descriptor.caller_supplied_constants(),
                "failure_modes": descriptor.failure_modes(),
                "supported_catalogue_claims": supported_claims,
            },
            "guarantees": [
                "measurand, resolution, imputation, and evidence-design checks retain their first typed refusal",
                "undeclared resolution is not silently treated as unresolved or resolved",
                "pseudoreplication is audited separately from whether the modality can support the claim",
                "custom descriptors remain bound to the requested modality and are never merged silently",
            ],
            "limitations": [
                "support means the modality descriptor is structurally eligible; it does not establish truth, power, effect size, or statistical validity",
                "the catalogue describes general modality contracts; study-specific evidence must be supplied through a custom descriptor",
                "failure modes marked not mechanised remain visible but are not detected by this check",
            ],
        }))
    }

    pub(super) fn modality_transport_check(&self, arguments: &Value) -> Result<Value, String> {
        let from: Modality = serde_json::from_value(
            arguments
                .get("from")
                .cloned()
                .ok_or("from is required and must be a serialized Modality")?,
        )
        .map_err(|error| format!("invalid source modality: {error}"))?;
        let to: Modality = serde_json::from_value(
            arguments
                .get("to")
                .cloned()
                .ok_or("to is required and must be a serialized Modality")?,
        )
        .map_err(|error| format!("invalid destination modality: {error}"))?;
        let axis: Resolution = serde_json::from_value(
            arguments
                .get("axis")
                .cloned()
                .ok_or("axis is required and must be a serialized Resolution")?,
        )
        .map_err(|error| format!("invalid transported resolution axis: {error}"))?;
        let source: ModalityDescriptor = match arguments.get("source_descriptor") {
            None | Some(Value::Null) => bioprism_modalities::catalog::descriptor(from),
            Some(value) => serde_json::from_value(value.clone())
                .map_err(|error| format!("invalid source modality descriptor: {error}"))?,
        };
        if source.modality != from {
            return Err(format!(
                "source descriptor modality {:?} does not match from {:?}",
                source.modality, from
            ));
        }
        if source.purpose.len() > 100_000 {
            return Err(
                "source modality descriptor purpose exceeds the 100000-byte safety bound".into(),
            );
        }
        let kind: TransportKind = serde_json::from_value(
            arguments
                .get("transport")
                .cloned()
                .ok_or("transport is required and must be a serialized TransportKind")?,
        )
        .map_err(|error| format!("invalid modality transport kind: {error}"))?;
        let claims: Vec<ClaimKind> = arguments
            .get("claims")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .cloned()
                    .map(serde_json::from_value)
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()
            .map_err(|error| format!("invalid modality transport claim kind: {error}"))?
            .unwrap_or_default();
        if claims.len() > ClaimKind::ALL.len() {
            return Err("modality transport claim checks exceed the 20-claim safety bound".into());
        }
        let transport = match &kind {
            TransportKind::Aggregation { operator } => {
                ModalityTransport::aggregating(&source, to, axis, *operator)
            }
            TransportKind::Deconvolution {
                reference,
                recomposition,
            } => ModalityTransport::deconvolving(
                &source,
                to,
                axis,
                reference.clone(),
                *recomposition,
            ),
            TransportKind::Imputation { model } => {
                ModalityTransport::imputing(&source, to, axis, model.clone())
            }
        };
        let refusal_projection = |refusal: &bioprism_modalities::TransportRefusal| {
            serde_json::to_value(refusal)
                .map(|value| {
                    json!({
                        "refusal": value.clone(),
                        "refusal_kind": value.get("transport_refusal").cloned(),
                        "refusal_text": refusal.to_string(),
                    })
                })
                .map_err(|error| format!("cannot serialize modality transport refusal: {error}"))
        };
        let support_row = |descriptor: &ModalityDescriptor,
                           claim: ClaimKind|
         -> Result<Value, String> {
            match modality_supports_descriptor(descriptor, claim) {
                Ok(()) => Ok(json!({
                    "claim": claim,
                    "supported": true,
                    "refusal": Value::Null,
                    "refusal_kind": Value::Null,
                    "refusal_text": Value::Null,
                })),
                Err(refusal) => {
                    let value = serde_json::to_value(&refusal).map_err(|error| {
                        format!("cannot serialize claim support refusal after transport: {error}")
                    })?;
                    Ok(json!({
                        "claim": claim,
                        "supported": false,
                        "refusal": value.clone(),
                        "refusal_kind": value.get("unsupported").cloned(),
                        "refusal_text": refusal.to_string(),
                    }))
                }
            }
        };
        match transport {
            Err(refusal) => {
                let projection = refusal_projection(&refusal)?;
                Ok(json!({
                    "ok": true,
                    "schema": "bioprism-mcp/modality-transport-check/0.1",
                    "outcome_kind": "refused",
                    "constructed": false,
                    "from": from,
                    "to": to,
                    "axis": axis,
                    "transport": kind,
                    "source_descriptor": {
                        "modality": source.modality,
                        "measurand": source.measurand,
                        "design": source.design,
                        "resolution": Resolution::ALL.into_iter().map(|axis| json!({"axis": axis, "status": source.resolution(axis)})).collect::<Vec<_>>(),
                    },
                    "transport_evidence": projection,
                    "claims": [],
                    "guarantees": [
                        "transport constructors refuse unnamed bases and source-axis violations",
                        "failed construction never emits a partial transported descriptor",
                    ],
                    "limitations": [
                        "no values are moved and no estimator or arithmetic is executed",
                    ],
                }))
            }
            Ok(transport) => {
                let mapped = transport.to_scope_mapping();
                let mapping_check = match mapped.check() {
                    bioprism_scope::MappingCheck::Sound => "sound",
                    bioprism_scope::MappingCheck::MisdeclaredRestriction => {
                        "misdeclared_restriction"
                    }
                    bioprism_scope::MappingCheck::UndeclaredLoss => "undeclared_loss",
                };
                let inverse = match transport.invert() {
                    Ok(inverse) => json!({
                        "invertible": true,
                        "inverse": serde_json::to_value(inverse).map_err(|error| error.to_string())?,
                        "refusal": Value::Null,
                        "refusal_kind": Value::Null,
                        "refusal_text": Value::Null,
                    }),
                    Err(refusal) => {
                        let projection = refusal_projection(&refusal)?;
                        json!({
                            "invertible": false,
                            "inverse": Value::Null,
                            "refusal": projection["refusal"].clone(),
                            "refusal_kind": projection["refusal_kind"].clone(),
                            "refusal_text": projection["refusal_text"].clone(),
                        })
                    }
                };
                let applied = transport.apply(&source);
                let (applied_descriptor, application, claim_rows) = match applied {
                    Ok(descriptor) => {
                        let claim_rows = claims
                            .iter()
                            .map(|claim| {
                                let before = support_row(&source, *claim)?;
                                let after = support_row(&descriptor, *claim)?;
                                Ok(json!({
                                    "claim": claim,
                                    "before": before,
                                    "after": after,
                                    "support_lost": before["supported"] == json!(true) && after["supported"] == json!(false),
                                    "support_gained": before["supported"] == json!(false) && after["supported"] == json!(true),
                                }))
                            })
                            .collect::<Result<Vec<_>, String>>()?;
                        (
                            json!({
                                "descriptor": {
                                    "modality": descriptor.modality,
                                    "measurand": descriptor.measurand,
                                    "design": descriptor.design,
                                    "complete": descriptor.is_complete(),
                                    "resolutions": Resolution::ALL.into_iter().map(|axis| json!({"axis": axis, "status": descriptor.resolution(axis)})).collect::<Vec<_>>(),
                                    "resolved_axes": descriptor.resolved_axes(),
                                    "unresolved_axes": descriptor.unresolved_axes(),
                                    "undeclared_axes": descriptor.undeclared_axes(),
                                }
                            }),
                            json!({"applied": true, "refusal": Value::Null, "refusal_kind": Value::Null, "refusal_text": Value::Null}),
                            claim_rows,
                        )
                    }
                    Err(refusal) => {
                        let projection = refusal_projection(&refusal)?;
                        (
                            Value::Null,
                            json!({"applied": false, "refusal": projection["refusal"].clone(), "refusal_kind": projection["refusal_kind"].clone(), "refusal_text": projection["refusal_text"].clone()}),
                            Vec::new(),
                        )
                    }
                };
                Ok(json!({
                    "ok": true,
                    "schema": "bioprism-mcp/modality-transport-check/0.1",
                    "outcome_kind": "constructed",
                    "constructed": true,
                    "from": from,
                    "to": to,
                    "axis": axis,
                    "transport": serde_json::to_value(&transport).map_err(|error| error.to_string())?,
                    "fidelity": serde_json::to_value(transport.fidelity()).map_err(|error| error.to_string())?,
                    "loss": serde_json::to_value(transport.loss()).map_err(|error| error.to_string())?,
                    "scope_mapping": serde_json::to_value(&mapped).map_err(|error| error.to_string())?,
                    "scope_mapping_check": mapping_check,
                    "inverse": inverse,
                    "application": application,
                    "applied_descriptor": applied_descriptor,
                    "claims": claim_rows,
                    "guarantees": [
                        "loss ledgers travel with every constructed transport",
                        "exact aggregation is distinct from estimated deconvolution and imputation",
                        "invertibility is reported separately from fidelity and never implies value recovery",
                        "claim support is compared before and after the resolution change rather than inferred",
                    ],
                    "limitations": [
                        "the workflow audits declarations only; it does not move values, fit models, or validate a reference panel",
                        "a successful descriptor transition does not establish transport accuracy or biological equivalence",
                        "a round-trip or inverse mapping remains a structural operation, not proof that discarded information was recovered",
                    ],
                }))
            }
        }
    }

    pub(super) fn modality_comparability_check(&self, arguments: &Value) -> Result<Value, String> {
        let left: ModalMeasurement = serde_json::from_value(
            arguments
                .get("left")
                .cloned()
                .ok_or("left is required and must be a serialized ModalMeasurement")?,
        )
        .map_err(|error| format!("invalid left modal measurement: {error}"))?;
        let right: ModalMeasurement = serde_json::from_value(
            arguments
                .get("right")
                .cloned()
                .ok_or("right is required and must be a serialized ModalMeasurement")?,
        )
        .map_err(|error| format!("invalid right modal measurement: {error}"))?;
        let policy: bioprism_standards::ComparabilityPolicy = arguments
            .get("policy")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| format!("invalid modality comparability policy: {error}"))?
            .unwrap_or_default();
        let report = modality_comparability_report(&left, &right, policy);
        let verdict = serde_json::to_value(&report.verdict)
            .map_err(|error| format!("cannot serialize cross-modal verdict: {error}"))?;
        let digest = report
            .digest()
            .map_err(|error| format!("cannot digest cross-modal report: {error}"))?;
        let side_summary = |measurement: &ModalMeasurement| {
            json!({
                "modality": measurement.modality(),
                "measurand": measurement.measurand(),
                "reported_at": measurement.reported_at,
                "axis_status": measurement.axis_status(),
                "measurement": &measurement.measurement,
            })
        };
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/modality-comparability-check/0.1",
            "outcome_kind": if report.verdict.is_comparable() { "comparable" } else { "blocked" },
            "comparable": report.verdict.is_comparable(),
            "policy": policy,
            "check_order": [
                "measurand",
                "reported resolution axis",
                "status of that axis",
                "everything in bioprism_standards::CHECK_ORDER",
            ],
            "left": side_summary(&left),
            "right": side_summary(&right),
            "report": report,
            "verdict": verdict,
            "report_sha256": digest.to_string(),
            "guarantees": [
                "measurand and modality-resolution checks run before delegated unit, frame, build, and ontology checks",
                "a standards-comparable pair can still be blocked as different biological quantities",
                "imputed axes are not silently treated as measured axes",
                "a comparable verdict means category compatibility, not equality, correctness, power, or biological agreement",
            ],
            "limitations": [
                "no measurement values are read or statistically compared",
                "resolution-changing aggregation must be declared as a separate modality transport",
                "the standards report is absent when the modality layer blocks before delegation",
            ],
        }))
    }

    pub(super) fn literature_bind_check(&self, arguments: &Value) -> Result<Value, String> {
        let claim: LiteratureClaim = serde_json::from_value(
            arguments
                .get("claim")
                .cloned()
                .ok_or("claim is required and must be a serialized LiteratureClaim")?,
        )
        .map_err(|error| format!("invalid literature claim: {error}"))?;
        if claim.text.len() > 100_000 {
            return Err("literature claim text exceeds the 100000-byte safety bound".into());
        }
        if claim.provenance.identifier.is_empty() || claim.provenance.identifier.len() > 2_000 {
            return Err(
                "literature source identifier must contain between 1 and 2000 bytes".into(),
            );
        }
        let target: ScopeKey = serde_json::from_value(
            arguments
                .get("target")
                .cloned()
                .ok_or("target is required and must be a serialized ScopeKey")?,
        )
        .map_err(|error| format!("invalid literature target scope: {error}"))?;
        if target.len() > 100 {
            return Err("literature target scope may contain at most 100 dimensions".into());
        }
        let at_tier: EvidenceTier = serde_json::from_value(
            arguments
                .get("at_tier")
                .cloned()
                .ok_or("at_tier is required and must be an EvidenceTier")?,
        )
        .map_err(|error| format!("invalid literature citation tier: {error}"))?;
        let horizon: EvaluationHorizon = serde_json::from_value(
            arguments
                .get("horizon")
                .cloned()
                .ok_or("horizon is required and must be an EvaluationHorizon")?,
        )
        .map_err(|error| format!("invalid literature evaluation horizon: {error}"))?;
        let warrant = arguments.get("flag_warrant").and_then(Value::as_str);
        if warrant.is_some_and(|text| text.trim().is_empty()) {
            return Err("flag_warrant must not be empty when supplied".into());
        }
        if warrant.is_some_and(|text| text.len() > 10_000) {
            return Err("flag_warrant exceeds the 10000-byte safety bound".into());
        }
        let claim_kind: Option<ClaimKind> = arguments
            .get("claim_kind")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| format!("invalid literature claim kind: {error}"))?;
        let claim_value = serde_json::to_value(&claim).map_err(|error| error.to_string())?;
        let provenance_value =
            serde_json::to_value(&claim.provenance).map_err(|error| error.to_string())?;
        let target_value = serde_json::to_value(&target).map_err(|error| error.to_string())?;
        let horizon_value = serde_json::to_value(horizon).map_err(|error| error.to_string())?;
        let bind = match warrant {
            Some(warrant) => claim.bind_despite_flag(&target, at_tier, horizon, warrant),
            None => claim.bind(&target, at_tier, horizon),
        };
        let mut projection = json!({
            "ok": true,
            "schema": "bioprism-mcp/literature-bind-check/0.1",
            "claim": claim_value,
            "provenance": provenance_value,
            "target": target_value,
            "requested_tier": at_tier,
            "horizon": horizon_value,
            "flag_warrant_supplied": warrant.is_some(),
            "claim_kind": claim_kind,
            "bound": false,
            "citable": Value::Null,
            "bound_claim": Value::Null,
            "refusal": Value::Null,
            "refusal_kind": Value::Null,
            "citation": Value::Null,
            "citation_refusal": Value::Null,
            "citation_refusal_kind": Value::Null
        });
        match bind {
            Ok(bound) => {
                projection["bound"] = json!(true);
                projection["bound_claim"] =
                    serde_json::to_value(&bound).map_err(|error| error.to_string())?;
                projection["outcome_kind"] = json!("bound");
                if let Some(kind) = claim_kind {
                    match modality_cites(&bound, kind) {
                        Ok(tier) => {
                            projection["citable"] = json!(true);
                            projection["outcome_kind"] = json!("citable");
                            projection["citation"] = json!({
                                "claim_kind": kind,
                                "cited_as": tier,
                                "direct_evidence": tier.is_direct_evidence()
                            });
                        }
                        Err(refusal) => {
                            let refusal_value = serde_json::to_value(&refusal)
                                .map_err(|error| error.to_string())?;
                            projection["citable"] = json!(false);
                            projection["outcome_kind"] = json!("cite_refused");
                            projection["citation_refusal"] = refusal_value.clone();
                            projection["citation_refusal_kind"] = refusal_value
                                .get("unsupported")
                                .cloned()
                                .unwrap_or(Value::Null);
                            projection["citation_refusal_text"] = json!(refusal.to_string());
                        }
                    }
                }
            }
            Err(refusal) => {
                let refusal_value =
                    serde_json::to_value(&refusal).map_err(|error| error.to_string())?;
                projection["outcome_kind"] = json!("refused");
                projection["refusal"] = refusal_value.clone();
                projection["refusal_kind"] = refusal_value["binding_refusal"].clone();
                projection["refusal_text"] = json!(refusal.to_string());
            }
        }
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/literature-bind-check/0.1",
            "outcome_kind": projection["outcome_kind"],
            "bound": projection["bound"],
            "citable": projection["citable"],
            "evidence": projection,
            "guarantees": [
                "retraction, evaluation horizon, evidence tier, and target population are checked in the literature crate's declared order",
                "a review or guideline cannot be cited as primary evidence without changing the requested tier",
                "a bound literature claim remains a claim about a source and is not promoted into a measurement",
                "citation support is evaluated separately from successful scope binding"
            ],
            "limitations": [
                "no paper retrieval, identifier resolution, citation-graph traversal, natural-language entailment, or evidence synthesis is performed",
                "the target scope, source metadata, and any flagged-source warrant are caller-supplied"
            ]
        }))
    }
}
