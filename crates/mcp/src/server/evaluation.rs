//! Adaptive, oracle, reproduction, and federated evaluation handlers.

use super::*;

impl Server {
    pub(super) fn adaptive_panel(&self, arguments: &Value) -> Result<Value, String> {
        let panel: AdaptivePanel = serde_json::from_value(
            arguments
                .get("panel")
                .cloned()
                .ok_or("panel is required and must be a serialized AdaptivePanel")?,
        )
        .map_err(|error| format!("invalid adaptive panel: {error}"))?;
        if panel.ledger().len() > 100_000 {
            return Err("panel exceeds the 100000-trial safety bound".into());
        }
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if max_items == 0 || max_items > 1_000 {
            return Err("max_items must be between 1 and 1000".into());
        }
        let max_items = max_items as usize;

        let audit = match panel.audit() {
            Ok(audit) => audit,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "stage": "audit",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "guarantee": "coverage floors and stopping uncertainty are never converted into a reportable estimate"
                }));
            }
        };
        let audit_digest = audit
            .digest()
            .ok()
            .map(|digest| digest.as_str().to_string());
        let audit_summary = json!({
            "trials": audit.trials,
            "scored_trials": audit.scored_trials,
            "abstentions": audit.abstentions,
            "total_cost": audit.total_cost,
            "capabilities": audit.capabilities.len(),
            "reported": audit.reported(),
            "withheld": audit.withheld(),
            "effective_trials": audit.effective_trials(),
            "headline": audit.headline(),
        });

        let selection = if let Some(raw_candidates) = arguments.get("candidates") {
            let values = raw_candidates
                .as_array()
                .ok_or("candidates must be an array of adaptive Candidate values")?;
            if values.len() > 10_000 {
                return Err("candidates exceeds the 10000-candidate safety bound".into());
            }
            let candidates: Vec<AdaptiveCandidate> = values
                .iter()
                .cloned()
                .map(|value| {
                    serde_json::from_value(value)
                        .map_err(|error| format!("invalid adaptive candidate: {error}"))
                })
                .collect::<Result<_, _>>()?;
            let batch_size = arguments.get("batch_size").and_then(Value::as_u64);
            let outcome = if let Some(size) = batch_size {
                if size == 0 || size > 1_000 {
                    return Err("batch_size must be between 1 and 1000".into());
                }
                panel
                    .select_batch(&candidates, size as usize)
                    .map(|records| {
                        json!({
                            "mode": "batch",
                            "records": records.iter().take(max_items).collect::<Vec<_>>(),
                            "omitted": records.len().saturating_sub(max_items)
                        })
                    })
            } else {
                panel
                    .select_next(&candidates)
                    .map(|record| json!({ "mode": "next", "record": record }))
            };
            match outcome {
                Ok(value) => json!({ "ok": true, "value": value }),
                Err(error) => json!({
                    "ok": false,
                    "refusal": error.to_string(),
                    "fail_closed": true
                }),
            }
        } else if arguments.get("batch_size").is_some() {
            return Err("batch_size requires candidates".into());
        } else {
            Value::Null
        };

        let capability_view = if let Some(raw_capability) = arguments.get("capability") {
            let capability = raw_capability
                .as_str()
                .ok_or("capability must be a string")?;
            let capability = AdaptiveCapabilityId::parse(capability)
                .map_err(|error| format!("invalid adaptive capability: {error}"))?;
            let coverage = panel.coverage(&capability);
            let stopping = panel.stopping_verdict(&capability);
            let estimate = panel.estimate(&capability);
            json!({
                "capability": capability,
                "coverage": coverage,
                "stopping": stopping.as_ref().ok(),
                "stopping_refusal": stopping.as_ref().err().map(ToString::to_string),
                "estimate": estimate.as_ref().ok(),
                "estimate_refusal": estimate.as_ref().err().map(ToString::to_string),
                "fail_closed": estimate.is_err()
            })
        } else {
            Value::Null
        };

        let comparison = match (
            arguments.get("left").and_then(Value::as_str),
            arguments.get("right").and_then(Value::as_str),
        ) {
            (None, None) => Value::Null,
            (Some(left), Some(right)) => {
                let left = AdaptiveCapabilityId::parse(left)
                    .map_err(|error| format!("invalid left capability: {error}"))?;
                let right = AdaptiveCapabilityId::parse(right)
                    .map_err(|error| format!("invalid right capability: {error}"))?;
                match panel.compare(&left, &right) {
                    Ok(value) => json!({ "ok": true, "value": value }),
                    Err(error) => json!({
                        "ok": false,
                        "refusal": error.to_string(),
                        "fail_closed": true
                    }),
                }
            }
            _ => return Err("left and right must be supplied together for comparison".into()),
        };
        let finished = panel.finished();

        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/adaptive-panel/0.1",
            "audit": audit,
            "audit_summary": audit_summary,
            "audit_digest": audit_digest,
            "selection": selection,
            "capability": capability_view,
            "comparison": comparison,
            "finished": finished.as_ref().ok(),
            "finished_refusal": finished.as_ref().err().map(ToString::to_string),
            "guarantees": [
                "abstentions are retained and costed but never counted as failures",
                "coverage floors, parent clustering, and stopping uncertainty remain explicit",
                "selection records preserve the clustered objective and coverage gate rather than a hidden random choice",
            ],
            "limitations": [
                "the panel consumes caller-supplied executed trials and predicted candidate costs",
                "the model has one parent-clustering level, no item model, no anytime-valid confidence sequence, and no exploration reserve",
                "this tool selects and audits; it does not execute candidates or establish release superiority over a fixed reference panel",
            ]
        }))
    }

    pub(super) fn posterior_gate(&self, arguments: &Value) -> Result<Value, String> {
        let raw_observations = arguments.get("observations").cloned().ok_or(
            "observations is required and must be an array of evalengine Observation values",
        )?;
        let values = raw_observations
            .as_array()
            .ok_or("observations must be an array")?;
        if values.len() > 10_000 {
            return Err("observations exceeds the 10000-observation safety bound".into());
        }
        let observations: Vec<Observation> = values
            .iter()
            .cloned()
            .map(|value| {
                serde_json::from_value(value)
                    .map_err(|error| format!("invalid evaluation observation: {error}"))
            })
            .collect::<Result<_, _>>()?;
        let policy: CreditPolicy = arguments
            .get("credit_policy")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| format!("invalid credit policy: {error}"))?
            .unwrap_or_default();
        if !policy.validate() {
            return Ok(json!({
                "ok": false,
                "schema": "bioprism-mcp/posterior-gate/0.1",
                "stage": "credit_policy",
                "refusal": "unsupported and contradicted credit ceilings must both be finite values in [0,1)",
                "fail_closed": true
            }));
        }
        let posterior = match CapabilityPosterior::build(&observations, &policy) {
            Ok(posterior) => posterior,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/posterior-gate/0.1",
                    "stage": "posterior",
                    "refusal": error.to_string(),
                    "fail_closed": true
                }));
            }
        };
        if posterior.capabilities.len() > 1_000 {
            return Err("posterior exceeds the 1000-capability safety bound".into());
        }
        let unprovenanced = bioprism_evalengine::unprovenanced(&observations).len();

        let gate = if let Some(raw_gate) = arguments.get("gate") {
            let gate: EvalReleaseGate = serde_json::from_value(raw_gate.clone())
                .map_err(|error| format!("invalid release gate: {error}"))?;
            match posterior.overall(&gate) {
                Ok(value) => json!({ "ok": true, "value": value }),
                Err(error) => json!({
                    "ok": false,
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "guarantee": "a scalar is available only behind an explicit rationale, coverage floors, evidence-tier requirements, and veto checks"
                }),
            }
        } else {
            Value::Null
        };

        let comparison = if let Some(raw_other) = arguments.get("other_observations") {
            let values = raw_other
                .as_array()
                .ok_or("other_observations must be an array")?;
            if values.len() > 10_000 {
                return Err("other_observations exceeds the 10000-observation safety bound".into());
            }
            let other: Vec<Observation> = values
                .iter()
                .cloned()
                .map(|value| {
                    serde_json::from_value(value)
                        .map_err(|error| format!("invalid comparison observation: {error}"))
                })
                .collect::<Result<_, _>>()?;
            let other = match CapabilityPosterior::build(&other, &policy) {
                Ok(other) => other,
                Err(error) => {
                    return Ok(json!({
                        "ok": false,
                        "schema": "bioprism-mcp/posterior-gate/0.1",
                        "stage": "comparison_posterior",
                        "refusal": error.to_string(),
                        "fail_closed": true
                    }));
                }
            };
            let tolerance = arguments
                .get("tolerance")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let min_effective = arguments
                .get("min_effective")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            if !tolerance.is_finite()
                || tolerance < 0.0
                || !min_effective.is_finite()
                || min_effective < 0.0
            {
                return Err("tolerance and min_effective must be finite and non-negative".into());
            }
            json!({
                "ok": true,
                "dominance": posterior.compare(&other, tolerance, min_effective),
                "tolerance": tolerance,
                "min_effective": min_effective
            })
        } else {
            Value::Null
        };

        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/posterior-gate/0.1",
            "schema_version": posterior.schema_version,
            "observations": observations.len(),
            "unprovenanced_observations": unprovenanced,
            "capabilities": posterior.capabilities,
            "gate": gate,
            "comparison": comparison,
            "guarantees": [
                "capability vectors preserve pass rate, outcome rate, partial credit, unknown share, vetoes, disputes, and weakest evidence tier separately",
                "unmeasured or thin capabilities remain incomparable rather than becoming zero or a forced tie",
                "release scalars carry their rationale, formula, terms, and leave-one-out sensitivity",
            ],
            "limitations": [
                "observations and provenance are caller-supplied; the tool does not execute evaluators or verify external run handles",
                "the posterior is a clustered point-estimate vector, not a fitted probability distribution",
                "cost, latency, calibration curves, failure-atlas clustering, and human dispute resolution remain outside this contract",
            ]
        }))
    }

    pub(super) fn oracle_reference_panel(&self, arguments: &Value) -> Result<Value, String> {
        let panel: ReaderPanel = serde_json::from_value(
            arguments
                .get("panel")
                .cloned()
                .ok_or("panel is required and must be a serialized ReaderPanel")?,
        )
        .map_err(|error| format!("invalid reader panel: {error}"))?;
        if panel.reads().len() > 10_000 {
            return Err("reader panel may contain at most 10000 reads".into());
        }
        let rule = arguments
            .get("rule")
            .cloned()
            .map(serde_json::from_value::<ConsensusRule>)
            .transpose()
            .map_err(|error| format!("invalid consensus rule: {error}"))?
            .unwrap_or(ConsensusRule::Majority);
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if max_items == 0 || max_items > 1_000 {
            return Err("max_items must be between 1 and 1000".into());
        }
        let max_items = max_items as usize;
        let reference = match panel.reference(rule) {
            Ok(reference) => reference,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "stage": "reference",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "guarantee": "an empty independent panel never becomes a reference by default",
                }));
            }
        };
        let model_call = arguments.get("model_call").and_then(Value::as_str);
        let rule_label = reference.rule.label();
        Ok(json!({
            "ok": true,
            "rule": reference.rule,
            "rule_label": rule_label,
            "consensus": reference.consensus(),
            "tally": reference.tally(),
            "readers": reference.readers(),
            "minority_calls": reference.minority_calls(),
            "reads": reference.reads().iter().take(max_items).collect::<Vec<_>>(),
            "omitted_reads": reference.reads().len().saturating_sub(max_items),
            "per_reader": model_call.map(|call| reference.per_reader(call)),
            "model_call": model_call,
            "adjudication": panel.adjudication(),
            "guarantees": [
                "only independent pre-discussion reads count toward the consensus rule",
                "minority calls and all source reads remain visible even when a reference is settled",
                "split panels and unblinded adjudication remain unresolved rather than defaulting to the first or majority call",
            ],
            "limitations": [
                "reader calls, evidence citations, and adjudication blinding are caller-supplied and not independently audited",
                "the tool reports raw agreement structure; it does not invent an agreement statistic or calibration model",
                "a reference distribution is a measurement process claim, not a claim of biological truth",
            ],
        }))
    }

    pub(super) fn oracle_missingness(&self, arguments: &Value) -> Result<Value, String> {
        let pattern: AbsencePattern = serde_json::from_value(
            arguments
                .get("pattern")
                .cloned()
                .ok_or("pattern is required and must be a serialized AbsencePattern")?,
        )
        .map_err(|error| format!("invalid absence pattern: {error}"))?;
        if pattern.groups().count() > 10_000 {
            return Err("absence pattern may contain at most 10000 groups".into());
        }
        let field: Field = serde_json::from_value(
            arguments
                .get("field")
                .cloned()
                .ok_or("field is required and must be a serialized Field")?,
        )
        .map_err(|error| format!("invalid missingness field: {error}"))?;
        let boundary: Boundary = serde_json::from_value(
            arguments
                .get("boundary")
                .cloned()
                .ok_or("boundary is required and must be a serialized Boundary")?,
        )
        .map_err(|error| format!("invalid missingness boundary: {error}"))?;
        let small_cell_floor = arguments
            .get("small_cell_floor")
            .and_then(Value::as_u64)
            .ok_or("small_cell_floor is required and must be a non-negative integer")?;
        if small_cell_floor > 1_000_000_000 {
            return Err("small_cell_floor is above the 1000000000 safety bound".into());
        }
        let mechanism = arguments
            .get("mechanism")
            .cloned()
            .map(serde_json::from_value::<MissingnessMechanism>)
            .transpose()
            .map_err(|error| format!("invalid missingness mechanism: {error}"))?;
        let complete_case = mechanism.as_ref().map(complete_case_admissible);
        Ok(json!({
            "ok": true,
            "groups": pattern.groups().collect::<Vec<_>>(),
            "informativeness": informativeness(&pattern),
            "field": field,
            "boundary": boundary,
            "small_cell_floor": small_cell_floor,
            "egress": oraclex_egress(&field, &boundary, small_cell_floor),
            "mechanism": mechanism,
            "complete_case": complete_case,
            "guarantees": [
                "absence informativeness distinguishes deterministic separation, unresolved partial separation, and equal supplied rates",
                "individual data, small aggregates, and denominator-free aggregates are not silently released across an aggregate boundary",
                "complete-case admissibility remains unresolved until a missingness mechanism is declared",
            ],
            "limitations": [
                "counts and missingness mechanisms are caller-supplied; no imputation, causal model, or external audit is performed",
                "the small-cell floor is a required policy input and is not a universal biological or legal threshold",
                "egress is a determination over a value, not a network or storage access-control enforcement point",
            ],
        }))
    }

    pub(super) fn evaluation_worldline_audit(&self, arguments: &Value) -> Result<Value, String> {
        let worldline: EvaluationWorldline = serde_json::from_value(
            arguments
                .get("worldline")
                .cloned()
                .ok_or("worldline is required and must be a serialized bioevalx Worldline")?,
        )
        .map_err(|error| format!("invalid evaluation worldline: {error}"))?;
        if worldline.decisions().len() > 10_000 {
            return Err("worldline exceeds the 10000-decision safety bound".into());
        }

        let audit = worldline.audit();
        let dangling = worldline.dangling();
        let admissible_at = match arguments.get("at") {
            None | Some(Value::Null) => Value::Null,
            Some(value) => {
                let at = value
                    .as_str()
                    .ok_or("at must be an RFC-3339 timestamp string")?;
                let at = bioprism_scope::Timestamp::parse(at)
                    .map_err(|error| format!("invalid at timestamp: {error}"))?;
                json!(worldline.admissible_at(at))
            }
        };

        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/evaluation-worldline-audit/0.1",
            "decisions": worldline.decisions().len(),
            "leak_count": audit.len(),
            "leaks": audit,
            "dangling_count": dangling.len(),
            "dangling_references": dangling,
            "admissible_at": admissible_at,
            "guarantees": [
                "future evidence and unresolved references are reported separately",
                "the audit is based on accessibility time rather than occurrence time",
                "the worldline contract keeps the numerator of leakage explicit instead of inventing a denominator",
            ],
            "limitations": [
                "a serialized worldline is caller-supplied and this tool does not reconstruct missing observations",
                "no leakage rate is emitted because the denominator must be chosen by the evaluation design",
            ]
        }))
    }

    pub(super) fn evaluation_reproduction_check(&self, arguments: &Value) -> Result<Value, String> {
        let reexecution: Reexecution = serde_json::from_value(
            arguments
                .get("reexecution")
                .cloned()
                .ok_or("reexecution is required and must be a serialized bioevalx Reexecution")?,
        )
        .map_err(|error| format!("invalid reexecution: {error}"))?;
        if reexecution.specs().len() > 10_000 {
            return Err("reexecution exceeds the 10000-output safety bound".into());
        }

        let certificate = match reexecution.certify() {
            Ok(certificate) => certificate,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/evaluation-reproduction-check/0.1",
                    "stage": "certification",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "guarantee": "an empty or malformed comparison set never becomes a reproducibility certificate"
                }));
            }
        };
        let verdicts: Vec<Value> = certificate
            .verdicts()
            .iter()
            .map(|(output, verdict)| {
                let mut record = serde_json::Map::new();
                record.insert("output".into(), json!(output));
                if let Value::Object(fields) =
                    serde_json::to_value(verdict).map_err(|error| error.to_string())?
                {
                    record.extend(fields);
                }
                Ok(Value::Object(record))
            })
            .collect::<Result<_, String>>()?;
        let matched_count = certificate
            .verdicts()
            .iter()
            .filter(|(_, verdict)| matches!(verdict, OutputVerdict::Matched))
            .count();
        let diverged_count = certificate
            .verdicts()
            .iter()
            .filter(|(_, verdict)| matches!(verdict, OutputVerdict::Diverged { .. }))
            .count();
        let missing_count = certificate
            .verdicts()
            .iter()
            .filter(|(_, verdict)| matches!(verdict, OutputVerdict::Missing))
            .count();
        let first_divergence = certificate
            .first_divergence()
            .map(|(output, verdict)| json!({ "output": output, "verdict": verdict }));
        let validity_claim = arguments
            .get("biological_claim")
            .and_then(Value::as_str)
            .map(|claim| match certificate.supports(claim) {
                Ok(()) => json!({ "ok": true }),
                Err(error) => json!({
                    "ok": false,
                    "refusal": error.to_string(),
                    "fail_closed": true
                }),
            });

        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/evaluation-reproduction-check/0.1",
            "certificate": certificate,
            "verdicts": verdicts,
            "verdict_count": certificate.verdicts().len(),
            "matched_count": matched_count,
            "diverged_count": diverged_count,
            "missing_count": missing_count,
            "reproduced": certificate.reproduced(),
            "first_divergence": first_divergence,
            "missing_outputs": certificate.missing(),
            "portability_demonstrated": certificate.portability_demonstrated(),
            "validity_claim": validity_claim,
            "guarantees": [
                "missing outputs are distinct from numerical or digest divergence",
                "the first divergence remains in declared output order",
                "reproducibility is not promoted to biological validity",
                "the tolerance and environment-pinning declarations travel with the certificate",
            ],
            "limitations": [
                "the tool compares caller-supplied rerun observations and performs no execution",
                "environment reconstruction, figure regeneration, and statistical validity remain outside this contract",
            ]
        }))
    }

    pub(super) fn evaluation_observability_card(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a serialized EvaluationCardRequest")?;
        let receipt = crate::research_contracts::compile_evaluation_card_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_evalengine::EVALUATION_OBSERVABILITY_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "all declared baselines remain visible with deterministic counts and omissions",
                "cost-normalized auditable-discovery rate and Wilson uncertainty are emitted together",
                "under-sampled baselines block a production pass; the card does not claim biological validity"
            ],
            "limitations": [
                "the tool aggregates caller-supplied capability-run telemetry and does not inspect raw experimental data",
                "a pass is a measurement-gate result, not a clinical or biological decision"
            ]
        }))
    }

    pub(super) fn federated_evaluation_consensus(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a serialized FederatedEvaluationRequest")?;
        let receipt = crate::research_contracts::evaluate_federated_evaluation_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_evalengine::FEDERATED_EVALUATION_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "site cards are validated against one capability and benchmark world before comparison",
                "canonical card-digest disagreement remains contradictory evidence rather than being averaged away",
                "blocked or missing sites remain explicit and cannot satisfy the minimum-site consensus gate"
            ],
            "limitations": [
                "the route consumes caller-supplied aggregate EvaluationCards and does not inspect raw experimental data",
                "consensus is evidence agreement, not biological validity or a clinical decision"
            ]
        }))
    }

    /// Run the domain-neutral section-33 descriptive analytics kernel.
    ///
    /// This sits beside `metrics_profile_audit` rather than replacing it: the profile tool audits
    /// capability vectors and partial orders, while this tool audits measured scalar rows, paired
    /// contrasts, and probability forecasts. The input remains caller-supplied and the response
    /// says so explicitly; no causal, clinical, or universal claim is inferred here.
    pub(super) fn metrics_analytics_audit(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode metrics analytics input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("metrics analytics input exceeds the 20000000-byte safety bound".into());
        }
        let input: AnalyticsInput = serde_json::from_value(arguments.clone())
            .map_err(|error| format!("invalid metrics analytics input: {error}"))?;
        let report = analyse_analytics(&input)
            .map_err(|error| format!("metrics analytics refused: {error}"))?;
        let mut output = serde_json::to_value(report)
            .map_err(|error| format!("cannot encode metrics analytics report: {error}"))?;
        output["ok"] = json!(true);
        output["workflow"] = json!("metrics_descriptive_analytics");
        output["guarantees"] = json!([
            "observed and reproduced rows contribute to summaries; declared, missing, blocked, and not-applicable rows remain visible and excluded",
            "all domains share one bounded arithmetic kernel while dimension, direction, unit, and condition coordinates remain attached",
            "paired outputs are explicit descriptive contrasts suitable for robustness, cross-modal, translation, causal-design, or coordination review",
            "calibration outputs retain bins, Brier error, and expected calibration error instead of collapsing forecasts to a pass/fail label",
            "empty measured populations remain null summaries rather than zero performance",
        ]);
        output["limitations"] = json!([
            "the tool does not run evaluations, assays, models, agents, or acquisitions",
            "paired deltas and retention are not causal effects, clinical validation, or a universal score",
            "no missing-value imputation, dependency correction, confidence interval estimation, or external data access occurs",
            "calibration uses caller-supplied bounded outcomes and equal-width bins; it does not fit or validate a probabilistic model",
        ]);
        output["analytics_schema_version"] = json!(ANALYTICS_SCHEMA_VERSION);
        Ok(output)
    }

    pub(super) fn metrics_profile_audit(&self, arguments: &Value) -> Result<Value, String> {
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if max_items == 0 || max_items > 1_000 {
            return Err("max_items must be between 1 and 1000".into());
        }
        let max_items = max_items as usize;
        let raw_vectors = arguments
            .get("vectors")
            .and_then(Value::as_array)
            .ok_or("vectors is required and must contain at least two capability vectors")?;
        if raw_vectors.len() < 2 || raw_vectors.len() > 100 {
            return Err("vectors must contain between 2 and 100 capability vectors".into());
        }
        let vectors = raw_vectors
            .iter()
            .enumerate()
            .map(|(index, value)| {
                serde_json::from_value::<CapabilityVector>(value.clone())
                    .map_err(|error| format!("invalid capability vector at index {index}: {error}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let waiver_values = arguments
            .get("waived_dimensions")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut policy = MetricsComparabilityPolicy::strict();
        for (index, value) in waiver_values.iter().enumerate() {
            let dimension = value.as_str().ok_or_else(|| {
                format!("waived_dimensions[{index}] must be a string dimension name")
            })?;
            policy = policy.waiving(dimension);
        }
        let ranking = PartialRanking::over_under(vectors, &policy)
            .map_err(|error| format!("metrics profile refused: {error}"))?;
        let breakdown = metrics_breakdown(&ranking);
        let profile_rows = breakdown
            .iter()
            .take(max_items)
            .map(|row| {
                json!({
                    "capability": &row.capability,
                    "best": &row.best,
                    "measured_for": &row.measured_for,
                    "unmeasured_for": &row.unmeasured_for,
                    "measured_count": row.measured_for.len(),
                    "unmeasured_count": row.unmeasured_for.len(),
                    "lead_is_uncontested": row.lead_is_uncontested(),
                })
            })
            .collect::<Vec<_>>();
        let system_rows = ranking
            .vectors()
            .iter()
            .take(max_items)
            .map(|vector| {
                let best_on = breakdown
                    .iter()
                    .filter(|row| row.best.contains(&vector.system))
                    .map(|row| {
                        json!({
                            "capability": &row.capability,
                            "uncontested": row.lead_is_uncontested(),
                        })
                    })
                    .collect::<Vec<_>>();
                let unmeasured = breakdown
                    .iter()
                    .filter(|row| row.unmeasured_for.contains(&vector.system))
                    .map(|row| &row.capability)
                    .collect::<Vec<_>>();
                let unmeasured_count = unmeasured.len();
                json!({
                    "system": &vector.system,
                    "grid": vector.grid.label,
                    "capability_count": vector.grid.capabilities().count(),
                    "measured_cell_count": vector.grid.measured().count(),
                    "hole_count": vector.grid.holes().count(),
                    "best_on": best_on,
                    "unmeasured_capabilities": unmeasured,
                    "unmeasured_capability_count": unmeasured_count,
                })
            })
            .collect::<Vec<_>>();
        let relation_count = ranking.relations().len();
        let unresolved_count = ranking.unresolved().len();
        let measured_capability_count = breakdown
            .iter()
            .filter(|row| !row.measured_for.is_empty())
            .count();
        let uncontested_count = breakdown
            .iter()
            .filter(|row| row.lead_is_uncontested())
            .count();
        let mut output = json!({
            "ok": true,
            "metrics_schema_version": METRICS_SCHEMA_VERSION,
            "max_items": max_items,
            "waived_dimensions": policy.waived().collect::<Vec<_>>(),
            "summary": {
                "system_count": ranking.vectors().len(),
                "capability_count": breakdown.len(),
                "capabilities_with_measurements": measured_capability_count,
                "capabilities_without_measurements": breakdown.len().saturating_sub(measured_capability_count),
                "uncontested_lead_count": uncontested_count,
                "relation_count": relation_count,
                "unresolved_count": unresolved_count,
                "is_total": ranking.is_total(),
                "maximal_systems": ranking.maximal(),
            },
            "per_capability": {
                "rows": profile_rows,
                "omitted_rows": breakdown.len().saturating_sub(max_items),
            },
            "per_system": {
                "rows": system_rows,
                "omitted_rows": ranking.vectors().len().saturating_sub(max_items),
            },
            "guarantees": [
                "a capability with no measurements remains visible as an empty lead rather than manufacturing a winner",
                "missing cells are reported separately from measured losses and are never scored as zero",
                "uncontested leads are labelled as evaluation findings rather than presented as validated wins",
                "the partial ranking remains available through the summary and preserves trade-offs and condition refusals",
            ],
            "limitations": [
                "this audits supplied metric vectors; it does not run evaluations or validate the scientific quality of their values",
                "best means the best supplied point estimate under the grid direction and does not perform interval or statistical dominance",
            ],
        });
        if let Some(raw_weighting) = arguments.get("weighting") {
            let weighting: DeclaredWeighting = serde_json::from_value(raw_weighting.clone())
                .map_err(|error| format!("invalid declared weighting: {error}"))?;
            let total = ranking
                .totalise(&weighting)
                .map_err(|error| format!("weighted profile refused: {error}"))?;
            let instability = RankInstability::measure(&ranking, &weighting)
                .map_err(|error| format!("weight sensitivity refused: {error}"))?;
            output["declared_weighting"] = json!({
                "intended_use": weighting.intended_use(),
                "digest": weighting.digest(),
                "capabilities": weighting.capabilities().collect::<Vec<_>>(),
                "weights": weighting.weights().map(|(capability, weight)| json!({
                    "capability": capability,
                    "weight": weight,
                })).collect::<Vec<_>>(),
            });
            output["total_order"] = json!({
                "leaders": total.leaders(),
                "overwrote_a_refusal": total.overwrote_a_refusal(),
                "order": total.order.iter().take(max_items).map(|row| json!({
                    "rank": row.rank,
                    "system": row.system,
                    "aggregate": row.aggregate,
                })).collect::<Vec<_>>(),
                "collapsed": total.collapsed.iter().take(max_items).map(|pair| json!({
                    "left": pair.left,
                    "right": pair.right,
                    "dominance": pair.dominance,
                })).collect::<Vec<_>>(),
                "omitted_order": total.order.len().saturating_sub(max_items),
                "omitted_collapsed": total.collapsed.len().saturating_sub(max_items),
            });
            output["rank_instability"] = json!(instability);
        }
        Ok(output)
    }

    /// Build an evidence-conditioned capability profile across the section-33 dimensions.
    ///
    /// The metric profile is necessary but not sufficient for a useful capability claim. This
    /// workflow keeps the score grid beside the evidence that makes a score interpretable:
    /// grounding, information value, resource denominator, temporal validity, cross-modal
    /// consistency, causal identification, re-execution, translation, and coordination. Existing
    /// typed contracts are invoked for the quantitative sub-audits; the evidence matrix is an
    /// explicit caller-supplied inventory whose missingness and supporting-field requirements are
    /// checked here.
    pub(super) fn biocapability_evidence_audit(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot measure biocapability audit envelope: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("biocapability audit input exceeds the 20000000-byte safety bound".into());
        }
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if !(1..=1_000).contains(&max_items) {
            return Err("max_items must be between 1 and 1000".into());
        }
        let max_items = max_items as usize;
        let evidence_inputs = input_array(arguments, "evidence", 512)?;
        let claim_inputs = input_array(arguments, "claim_requests", 128)?;

        let known_dimensions = BIOCAPABILITY_EVIDENCE_DIMENSIONS
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut seen_ids = BTreeSet::new();
        let mut states: BTreeMap<String, Vec<EvidenceState>> = BIOCAPABILITY_EVIDENCE_DIMENSIONS
            .iter()
            .map(|dimension| ((*dimension).to_string(), Vec::new()))
            .collect();
        let mut domains: BTreeMap<String, usize> = BTreeMap::new();
        let mut evidence_rows = Vec::with_capacity(evidence_inputs.len());

        for (index, raw) in evidence_inputs.iter().enumerate() {
            let row = match raw.as_object() {
                Some(object) => {
                    let parsed = (|| -> Result<Value, String> {
                        let id = required_string(object, "id")?;
                        if !seen_ids.insert(id.clone()) {
                            return Err(format!("duplicate evidence id {id:?}"));
                        }
                        let dimension = required_string(object, "dimension")?;
                        if !known_dimensions.contains(dimension.as_str()) {
                            return Err(format!(
                                "unknown evidence dimension {dimension:?}; choose one of {}",
                                BIOCAPABILITY_EVIDENCE_DIMENSIONS.join(", ")
                            ));
                        }
                        let declared_status = parse_evidence_state(
                            object
                                .get("status")
                                .ok_or("evidence item requires status")?,
                        )?;
                        let domain = object
                            .get("domain")
                            .and_then(Value::as_str)
                            .filter(|value| !value.trim().is_empty())
                            .unwrap_or("unspecified")
                            .to_string();
                        let mut issues = Vec::new();
                        if declared_status.is_measured() {
                            validate_evidence_support(object, &dimension, &mut issues);
                        }
                        let effective_status = if issues.is_empty() {
                            declared_status
                        } else {
                            EvidenceState::Blocked
                        };
                        states
                            .entry(dimension.clone())
                            .or_default()
                            .push(effective_status);
                        *domains.entry(domain.clone()).or_default() += 1;
                        Ok(json!({
                            "index": index,
                            "ok": issues.is_empty(),
                            "id": id,
                            "dimension": dimension,
                            "domain": domain,
                            "declared_status": declared_status.as_str(),
                            "effective_status": effective_status.as_str(),
                            "issues": issues,
                            "support": object,
                            "fail_closed": !issues.is_empty(),
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
                    "refusal": "each evidence item must be an object",
                    "fail_closed": true,
                }),
            };
            evidence_rows.push(row);
        }

        let evidence_dimension_rows = BIOCAPABILITY_EVIDENCE_DIMENSIONS
            .iter()
            .map(|dimension| {
                let dimension_states = states.get(*dimension).expect("initialized above");
                let state = rollup_evidence_state(dimension_states);
                json!({
                    "dimension": dimension,
                    "state": state.as_str(),
                    "evidence_count": dimension_states.len(),
                    "measured_count": dimension_states.iter().filter(|state| state.is_measured()).count(),
                    "declared_count": dimension_states.iter().filter(|state| **state == EvidenceState::Declared).count(),
                    "blocked_count": dimension_states.iter().filter(|state| **state == EvidenceState::Blocked).count(),
                    "missing": dimension_states.is_empty() || matches!(state, EvidenceState::Missing),
                    "measured": state.is_measured(),
                })
            })
            .collect::<Vec<_>>();

        let mut claim_rows = Vec::with_capacity(claim_inputs.len());
        for (index, raw) in claim_inputs.iter().enumerate() {
            let row = match raw.as_object() {
                Some(object) => {
                    let parsed = (|| -> Result<Value, String> {
                        let id = required_string(object, "id")?;
                        let claim = required_string(object, "claim")?;
                        let required = object
                            .get("requires")
                            .ok_or("claim request requires a non-empty requires array")?;
                        let required = required
                            .as_array()
                            .ok_or("claim request requires a non-empty requires array")?;
                        if required.is_empty()
                            || required.len() > BIOCAPABILITY_EVIDENCE_DIMENSIONS.len()
                        {
                            return Err(format!(
                                "claim request {id:?} requires between 1 and {} dimensions",
                                BIOCAPABILITY_EVIDENCE_DIMENSIONS.len()
                            ));
                        }
                        let allow_declared = object
                            .get("allow_declared")
                            .and_then(Value::as_bool)
                            .unwrap_or(false);
                        let mut blockers = Vec::new();
                        let mut assumptions = Vec::new();
                        let mut required_dimensions = Vec::new();
                        for raw_dimension in required {
                            let dimension = raw_dimension
                                .as_str()
                                .ok_or("claim request dimensions must be strings")?;
                            if !known_dimensions.contains(dimension) {
                                return Err(format!(
                                    "claim request {id:?} names unknown dimension {dimension:?}"
                                ));
                            }
                            required_dimensions.push(dimension.to_string());
                            let dimension_states = states.get(dimension).expect("known dimension");
                            let state = rollup_evidence_state(dimension_states);
                            match state {
                                EvidenceState::Observed | EvidenceState::Reproduced => {}
                                EvidenceState::Declared if allow_declared => {
                                    assumptions.push(json!({
                                        "dimension": dimension,
                                        "assumption": "declared evidence accepted only as an explicit condition",
                                    }));
                                }
                                EvidenceState::Declared => blockers.push(json!({
                                    "dimension": dimension,
                                    "state": state.as_str(),
                                    "reason": "declared evidence is not a measured support for this claim",
                                })),
                                _ => blockers.push(json!({
                                    "dimension": dimension,
                                    "state": state.as_str(),
                                    "reason": "missing, blocked, or non-applicable evidence",
                                })),
                            }
                        }
                        Ok(json!({
                            "index": index,
                            "ok": true,
                            "id": id,
                            "claim": claim,
                            "requires": required_dimensions,
                            "allow_declared": allow_declared,
                            "eligible": blockers.is_empty(),
                            "blockers": blockers,
                            "explicit_assumptions": assumptions,
                            "fail_closed": !blockers.is_empty(),
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
                    "refusal": "each claim request must be an object",
                    "fail_closed": true,
                }),
            };
            claim_rows.push(row);
        }

        let metrics_arguments = arguments
            .get("metrics")
            .cloned()
            .unwrap_or_else(|| arguments.clone());
        let metrics = match self.metrics_profile_audit(&metrics_arguments) {
            Ok(value) => value,
            Err(error) => json!({
                "ok": false,
                "stage": "metrics_profile",
                "refusal": error,
                "fail_closed": true,
            }),
        };
        let metrics_ok = metrics.get("ok") == Some(&json!(true));

        let information_value = arguments.get("information").map(|raw| {
            match raw.as_object() {
                Some(_) => match self.epistemic_voi(raw) {
                    Ok(value) => value,
                    Err(error) => json!({
                        "ok": false,
                        "stage": "information_value",
                        "refusal": error,
                        "fail_closed": true,
                    }),
                },
                None => json!({
                    "ok": false,
                    "stage": "information_value",
                    "refusal": "information must be an object containing problem, belief, and acquisition(s)",
                    "fail_closed": true,
                }),
            }
        });

        let reference_quality = arguments.get("reference").map(|raw| {
            let mut subarguments = serde_json::Map::new();
            subarguments.insert("reference".into(), raw.clone());
            if let Some(state) = arguments.get("reference_state") {
                subarguments.insert("state".into(), state.clone());
            }
            match self.bioeval_reference_audit(&Value::Object(subarguments)) {
                Ok(value) => value,
                Err(error) => json!({
                    "ok": false,
                    "stage": "reference_quality",
                    "refusal": error,
                    "fail_closed": true,
                }),
            }
        });

        let temporal_validity = arguments.get("worldline").map(|raw| {
            let mut subarguments = serde_json::Map::new();
            subarguments.insert("worldline".into(), raw.clone());
            if let Some(at) = arguments.get("at") {
                subarguments.insert("at".into(), at.clone());
            }
            match self.evaluation_worldline_audit(&Value::Object(subarguments)) {
                Ok(value) => value,
                Err(error) => json!({
                    "ok": false,
                    "stage": "temporal_validity",
                    "refusal": error,
                    "fail_closed": true,
                }),
            }
        });

        let reproducibility = arguments.get("reexecution").map(|raw| {
            let mut subarguments = serde_json::Map::new();
            subarguments.insert("reexecution".into(), raw.clone());
            if let Some(claim) = arguments.get("biological_claim") {
                subarguments.insert("biological_claim".into(), claim.clone());
            }
            match self.evaluation_reproduction_check(&Value::Object(subarguments)) {
                Ok(value) => value,
                Err(error) => json!({
                    "ok": false,
                    "stage": "reproducibility",
                    "refusal": error,
                    "fail_closed": true,
                }),
            }
        });

        let claims_requested = claim_rows.len();
        let eligible_claims = claim_rows
            .iter()
            .filter(|row| row.get("eligible") == Some(&json!(true)))
            .count();
        let evidence_blockers = evidence_rows
            .iter()
            .filter(|row| row.get("fail_closed") == Some(&json!(true)))
            .count();
        let all_requested_claims_eligible =
            claims_requested > 0 && eligible_claims == claims_requested && evidence_blockers == 0;
        Ok(json!({
            "ok": true,
            "workflow": "biocapability_evidence_conditioned_profile",
            "metrics": metrics,
            "metrics_ok": metrics_ok,
            "evidence": {
                "items": evidence_rows.iter().take(max_items).cloned().collect::<Vec<_>>(),
                "omitted_items": evidence_rows.len().saturating_sub(max_items),
                "item_count": evidence_rows.len(),
                "invalid_item_count": evidence_blockers,
                "dimensions": evidence_dimension_rows,
                "domains": domains,
            },
            "claim_requests": {
                "rows": claim_rows.iter().take(max_items).cloned().collect::<Vec<_>>(),
                "omitted_rows": claim_rows.len().saturating_sub(max_items),
                "requested": claims_requested,
                "eligible": eligible_claims,
                "all_requested_claims_eligible": all_requested_claims_eligible,
            },
            "subaudits": {
                "information_value": information_value,
                "reference_quality": reference_quality,
                "temporal_validity": temporal_validity,
                "reproducibility": reproducibility,
            },
            "release_posture": {
                "ready_for_requested_claims": metrics_ok && all_requested_claims_eligible,
                "requires_explicit_claim_request": claims_requested == 0,
                "numeric_scores_are_not_claims_without_evidence": true,
                "declared_evidence_is_visible_but_not_measured_support": true,
            },
            "guarantees": [
                "capability scores remain conditional on their metric grid and comparability conditions",
                "evidence dimensions are rolled up with missing, declared, blocked, observed, and reproduced states kept distinct",
                "temporal, reproducibility, reference-truth, and information-value subaudits reuse their existing typed contracts",
                "claim readiness requires explicit requested dimensions and never promotes an empty inventory to a universal claim",
                "supporting-field omissions block measured evidence rather than being inferred from a status label",
            ],
            "limitations": [
                "evidence rows, domains, epochs, costs, and supporting fields are caller-supplied inventories; no external dataset or lab record is inspected",
                "the matrix is an audit of claim prerequisites, not a statistical estimator, causal identification engine, translation study, or multi-agent execution runtime",
                "the quantitative subaudits compare or price supplied objects; they do not run evaluations, assays, re-executions, or acquisition actions",
                "a ready posture means the declared contract is internally complete, not that a scientific or operational claim is true",
            ],
        }))
    }

    pub(super) fn scale_family_split_verify(&self, arguments: &Value) -> Result<Value, String> {
        let raw_corpus = arguments
            .get("corpus")
            .cloned()
            .ok_or("corpus is required and must be an array of GeneratedItem values")?;
        let corpus_items = raw_corpus
            .as_array()
            .ok_or("corpus must be an array of GeneratedItem values")?;
        if corpus_items.is_empty() || corpus_items.len() > 50_000 {
            return Err("corpus must contain between 1 and 50000 items".into());
        }
        let items: Vec<GeneratedItem> = serde_json::from_value(raw_corpus)
            .map_err(|error| format!("invalid scale corpus: {error}"))?;
        let mut corpus = Corpus::new();
        for item in items {
            corpus
                .insert(item)
                .map_err(|error| format!("invalid scale corpus: {error}"))?;
        }

        let raw_assignment = arguments.get("assignment").cloned().ok_or(
            "assignment is required and must map item ids to public, validation, or hidden",
        )?;
        let assignment_object = raw_assignment
            .as_object()
            .ok_or("assignment must be an object keyed by item id")?;
        if assignment_object.len() > 50_000 {
            return Err("assignment may contain at most 50000 item ids".into());
        }
        let assignment: BTreeMap<String, ScaleTier> = serde_json::from_value(raw_assignment)
            .map_err(|error| format!("invalid scale assignment: {error}"))?;
        let unknown: Vec<&String> = assignment
            .keys()
            .filter(|id| corpus.get(id).is_none())
            .take(10)
            .collect();
        if !unknown.is_empty() {
            return Err(format!(
                "assignment contains ids absent from the corpus: {}",
                unknown
                    .iter()
                    .map(|id| id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        let families = match corpus.families() {
            Ok(families) => families,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "stage": "lineage",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "guarantee": "a family split cannot be verified when lineage is cyclic or dangling"
                }));
            }
        };
        match verify_item_assignment(&corpus, &assignment) {
            Ok(report) => Ok(json!({
                "ok": true,
                "valid": true,
                "item_count": corpus.len(),
                "family_count": families.len(),
                "report": report,
                "guarantees": [
                    "the verification is over lineage roots, not surface similarity or item names",
                    "every corpus item must be assigned exactly once",
                    "a valid report keeps item counts and family counts separate",
                    "this endpoint verifies an imported assignment and never repairs or reassigns it",
                ],
            })),
            Err(error) => Ok(json!({
                "ok": false,
                "valid": false,
                "item_count": corpus.len(),
                "family_count": families.len(),
                "stage": "assignment",
                "refusal": error.to_string(),
                "fail_closed": true,
                "guarantee": "a family that straddles public, validation, and hidden cannot produce a release report"
            })),
        }
    }

    pub(super) fn analysis_qualify(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a serialized AnalysisQualificationRequest")?;
        let result = crate::research_contracts::qualify_analysis_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_evalengine::ANALYSIS_QUALIFICATION_FEATURE_ID,
            "receipt": result,
            "guarantees": [
                "candidate ordering is deterministic by declared score and candidate id",
                "estimand, assumptions, uncertainty, artifact coverage, and identification status are retained",
                "protected omissions and unidentified candidates cannot produce an unconditional qualification"
            ],
            "limitations": [
                "the route qualifies caller-declared candidates and does not fit a model or inspect raw data",
                "a qualified analysis result is not a biological, clinical, or treatment conclusion"
            ]
        }))
    }

    pub(super) fn protocol_matrix_simulate(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a serialized ProtocolMatrixRequest")?;
        let receipt = crate::research_contracts::simulate_protocol_matrix_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_lab::PROTOCOL_MATRIX_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "factor order is canonicalized before cell enumeration and receipt hashing",
                "network partitions, step failures, budget multipliers, retries, and compensation are simulated fail-closed",
                "the factorial matrix is bounded to 4096 cells and performs no physical instrument effect"
            ],
            "limitations": [
                "the route simulates caller-declared protocol steps and does not execute hardware or inspect raw study data",
                "a passing cell is a preflight simulation result, not evidence of scientific validity or operational success"
            ]
        }))
    }

    pub(super) fn multimodal_replication_evaluate(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a serialized MultimodalReplicationRequest")?;
        let report = crate::research_contracts::evaluate_multimodal_replication_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_evalengine::MULTIMODAL_REPLICATION_FEATURE_ID,
            "receipt": report,
            "guarantees": [
                "required modality schema, units, coordinates, QC, and preregistration gates are evaluated before aggregation",
                "incompatible or incomplete studies remain visible as omissions and contradictions rather than being averaged away",
                "raw imaging and omics bytes remain institution-local and only typed digests cross the contract"
            ],
            "limitations": [
                "the route evaluates caller-supplied manifests and does not inspect raw modality bytes or establish biological truth",
                "a replicated disposition is a comparability and evidence gate, not a clinical or treatment conclusion"
            ]
        }))
    }

    pub(super) fn quality_drift_evaluate(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a serialized QualityDriftRequest")?;
        let receipt = crate::research_contracts::evaluate_quality_drift_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::QUALITY_DRIFT_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "baseline and current QC metrics are compared deterministically with explicit deltas",
                "unmeasured metrics remain unknown and conformance or locality failures block release",
                "the route reads only typed local QC declarations and never exports raw experimental bytes"
            ],
            "limitations": [
                "the route evaluates caller-supplied metrics and does not recalculate QC from raw modality data",
                "stable metrics certify only the declared baseline tolerance, not biological validity"
            ]
        }))
    }

    pub(super) fn design_frontier_evaluate(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a serialized DesignFrontierRequest")?;
        let receipt = crate::research_contracts::evaluate_design_frontier_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_lab::DESIGN_FRONTIER_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "effect, variance, attrition, and budget scenarios are replayed through the deterministic design compiler",
                "scenario order is canonicalized and blocked cells retain compiler reasons",
                "the route performs local computation only and does not authorize laboratory execution"
            ],
            "limitations": [
                "scenario assumptions are caller-declared and no power model can establish biological truth",
                "a feasible design still requires independent policy, protocol, and instrument approvals"
            ]
        }))
    }

    pub(super) fn autonomy_batch_admit(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a serialized BatchAdmissionRequest")?;
        let receipt = crate::research_contracts::admit_autonomy_batch_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_policy::AUTONOMY_BATCH_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "every action is evaluated against the same validated grant and sorted by action id",
                "allowed, approval-required, and denied actions are all retained in one immutable receipt",
                "unknown or contradictory evidence cannot become autonomous permission"
            ],
            "limitations": [
                "the route admits policy actions and performs no runtime, instrument, network, or data effect",
                "approval-required actions still require the institution-local authority and signed preflight gates"
            ]
        }))
    }

    pub(super) fn workflow_batch_execute(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a serialized WorkflowBatchRequest")?;
        let receipt = crate::research_contracts::execute_workflow_batch_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_runtime::WORKFLOW_BATCH_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "workflow requests are canonicalized by workflow id and each receipt retains ordered nodes and reasons",
                "dry-run and execute modes are explicit and every blocked workflow remains visible",
                "the batch is content-addressed and replayable through the existing typed workflow executor"
            ],
            "limitations": [
                "execute mode does not bypass runtime grants, budgets, or declared-effect checks",
                "the route does not contact instruments or move protected raw data outside the institution"
            ]
        }))
    }

    pub(super) fn evaluation_trajectory_check(&self, arguments: &Value) -> Result<Value, String> {
        let trajectory: Trajectory = serde_json::from_value(
            arguments
                .get("trajectory")
                .cloned()
                .ok_or("trajectory is required and must be a serialized bioevalx Trajectory")?,
        )
        .map_err(|error| format!("invalid trajectory: {error}"))?;
        if trajectory.steps().len() > 100_000 {
            return Err("trajectory exceeds the 100000-step safety bound".into());
        }
        if trajectory.properties().len() > 10_000 {
            return Err("trajectory exceeds the 10000-property safety bound".into());
        }

        let bounded_suffix = match (
            arguments.get("step").and_then(Value::as_u64),
            arguments.get("horizon").and_then(Value::as_u64),
        ) {
            (None, None) => Value::Null,
            (Some(step), Some(horizon)) => {
                let step = usize::try_from(step).map_err(|_| "step is too large")?;
                let horizon = usize::try_from(horizon).map_err(|_| "horizon is too large")?;
                match trajectory.bounded_suffix(step, horizon) {
                    Ok(value) => {
                        json!({ "ok": true, "value": value, "complete": value.complete() })
                    }
                    Err(error) => json!({
                        "ok": false,
                        "refusal": error.to_string(),
                        "fail_closed": true
                    }),
                }
            }
            _ => return Err("step and horizon must be supplied together".into()),
        };

        let checked = trajectory.check();
        let property_outcomes: Vec<Value> = checked
            .iter()
            .map(|outcome| {
                json!({
                    "property": outcome.property,
                    "violations": outcome.violations,
                    "vacuous": outcome.vacuous,
                    "held": outcome.held(),
                })
            })
            .collect();
        let property_records: Vec<Value> = trajectory
            .properties()
            .iter()
            .map(|property| json!({ "name": property.name(), "property": property }))
            .collect();
        let recovery = trajectory.recovery();
        let recovery_records: Vec<Value> = recovery
            .iter()
            .map(|(failure_step, strategy_change_after)| {
                json!({
                    "failure_step": failure_step,
                    "strategy_change_after": strategy_change_after,
                    "latency": strategy_change_after.map(|step| step.saturating_sub(*failure_step)),
                })
            })
            .collect();
        let held_count = checked.iter().filter(|outcome| outcome.held()).count();
        let violated_count = checked
            .iter()
            .filter(|outcome| !outcome.violations.is_empty())
            .count();
        let vacuous_count = checked.iter().filter(|outcome| outcome.vacuous).count();

        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/evaluation-trajectory-check/0.1",
            "steps": trajectory.steps().len(),
            "acts": trajectory.acts(),
            "step_records": trajectory.steps(),
            "properties": trajectory.properties(),
            "property_records": property_records,
            "property_outcomes": property_outcomes,
            "property_count": checked.len(),
            "held_count": held_count,
            "violated_count": violated_count,
            "vacuous_count": vacuous_count,
            "recovery": recovery,
            "recovery_records": recovery_records,
            "recovery_count": recovery.len(),
            "bounded_suffix": bounded_suffix,
            "guarantees": [
                "violations retain step indices and vacuous properties are not credited as held",
                "immediate and downstream progress remain separate and a horizon is explicit",
                "recovery is reported as individual failure-to-strategy-change pairs rather than an average",
            ],
            "limitations": [
                "progress values and act labels are caller-supplied",
                "this contract evaluates declared path properties; it does not infer decision quality or partial observability",
            ]
        }))
    }
}
