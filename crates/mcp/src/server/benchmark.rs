//! MCP Benchmark construction and evaluation handlers.
//!
//! Kept beside the dispatcher so each domain surface can be reviewed independently.

use super::*;

impl Server {
    pub(super) fn benchmark_trace_analyze(&self, arguments: &Value) -> Result<Value, String> {
        let raw_failing = arguments
            .get("failing")
            .cloned()
            .ok_or("failing is required and must be a serialized Trace")?;
        let raw_reference = arguments.get("reference").cloned();
        let encoded = serde_json::to_vec(&json!({
            "failing": raw_failing.clone(),
            "reference": raw_reference.clone(),
        }))
        .map_err(|error| format!("cannot measure benchmark envelope: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("benchmark input exceeds the 20000000-byte safety bound".into());
        }
        let failing: TraceIr = serde_json::from_value(raw_failing)
            .map_err(|error| format!("invalid failing trace: {error}"))?;
        if failing.events.len() > 100_000 {
            return Err("failing trace is bounded at 100000 events".into());
        }
        let reference: Option<TraceIr> = raw_reference
            .map(|raw| {
                serde_json::from_value(raw)
                    .map_err(|error| format!("invalid reference trace: {error}"))
            })
            .transpose()?;
        if reference
            .as_ref()
            .is_some_and(|trace| trace.events.len() > 100_000)
        {
            return Err("reference trace is bounded at 100000 events".into());
        }

        let analysis = match analyse_benchmark(&failing, reference.as_ref()) {
            Ok(analysis) => analysis,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "stage": "benchmark_causal_analysis",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "guarantees": [
                        "empty or non-decision-bearing trajectories do not produce a fabricated cell",
                        "environment divergences are not relocated to a nearby agent step",
                    ],
                }));
            }
        };
        let boundaries = benchmark_boundaries(&failing, analysis.first_causal_step());
        let episodes = benchmark_episodes(&failing);
        let repetitions = benchmark_repetitions(&failing);
        Ok(json!({
            "ok": true,
            "trace_id": failing.trace_id,
            "succeeded": failing.succeeded,
            "event_count": failing.events.len(),
            "reference_trace_id": reference.as_ref().map(|trace| trace.trace_id.clone()),
            "analysis": analysis,
            "episodes": episodes,
            "boundaries": boundaries,
            "repetitions": repetitions,
            "summary": {
                "episode_count": episodes.len(),
                "boundary_count": boundaries.len(),
                "extractable_boundaries": boundaries.iter().filter(|boundary| boundary.extractable()).count(),
                "repetition_groups": repetitions.len(),
            },
            "guarantees": [
                "causal ranking, boundary ranking, repetition, and episode segmentation remain separate evidence layers",
                "observed textual divergence is not treated as intervention effect",
                "this endpoint proposes review material; it does not replay tools, fork an architecture, approve a cell, or package a benchmark",
            ],
        }))
    }

    pub(super) fn benchmark_decision_audit(&self, arguments: &Value) -> Result<Value, String> {
        let raw_trace = arguments
            .get("trace")
            .cloned()
            .ok_or("trace is required and must be a serialized Trace")?;
        let raw_reference = arguments.get("reference").cloned();
        let raw_actions = arguments
            .get("actions")
            .cloned()
            .unwrap_or_else(|| json!([]));
        let raw_constraints = arguments
            .get("constraints")
            .cloned()
            .unwrap_or_else(|| json!([]));
        let raw_claims = arguments
            .get("claims")
            .cloned()
            .unwrap_or_else(|| json!([]));
        let max_items = match arguments.get("max_items") {
            None => 100,
            Some(value) => value
                .as_u64()
                .ok_or("max_items must be a non-negative integer")?,
        };
        if max_items == 0 || max_items > 1_000 {
            return Err("max_items must be between 1 and 1000".into());
        }

        let encoded = serde_json::to_vec(&json!({
            "trace": raw_trace.clone(),
            "reference": raw_reference.clone(),
            "actions": raw_actions.clone(),
            "constraints": raw_constraints.clone(),
            "claims": raw_claims.clone(),
        }))
        .map_err(|error| format!("cannot measure benchmark decision-audit envelope: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err(
                "benchmark decision-audit input exceeds the 20000000-byte safety bound".into(),
            );
        }

        let trace: TraceIr =
            serde_json::from_value(raw_trace).map_err(|error| format!("invalid trace: {error}"))?;
        if trace.events.len() > 100_000 {
            return Err("trace is bounded at 100000 events".into());
        }
        let reference: Option<TraceIr> = raw_reference
            .map(|raw| {
                serde_json::from_value(raw)
                    .map_err(|error| format!("invalid reference trace: {error}"))
            })
            .transpose()?;
        if reference
            .as_ref()
            .is_some_and(|candidate| candidate.events.len() > 100_000)
        {
            return Err("reference trace is bounded at 100000 events".into());
        }

        let mut candidate_actions: Vec<BenchmarkCandidateAction> =
            serde_json::from_value(raw_actions)
                .map_err(|error| format!("invalid candidate action: {error}"))?;
        if candidate_actions.len() > 10_000 {
            return Err("candidate actions are bounded at 10000 items".into());
        }
        let constraints: Vec<BenchmarkConstraintRecord> =
            serde_json::from_value(raw_constraints)
                .map_err(|error| format!("invalid constraint record: {error}"))?;
        if constraints.len() > 10_000 {
            return Err("constraint records are bounded at 10000 items".into());
        }
        let claims: Vec<BenchmarkAssertion> = serde_json::from_value(raw_claims)
            .map_err(|error| format!("invalid assertion: {error}"))?;
        if claims.len() > 10_000 {
            return Err("assertions are bounded at 10000 items".into());
        }
        let evaluator_dispute = match arguments.get("evaluator_dispute") {
            None => None,
            Some(value) => Some(value.as_str().ok_or("evaluator_dispute must be a string")?),
        };

        let analysis = match analyse_benchmark(&trace, reference.as_ref()) {
            Ok(analysis) => analysis,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/benchmark-decision-audit/0.1",
                    "stage": "benchmark_causal_analysis",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "trace_id": trace.trace_id,
                    "guarantees": [
                        "a decision audit never manufactures a causal trajectory from an empty or non-decision-bearing trace",
                        "causal refusal is preserved instead of being converted into agent blame",
                    ],
                }));
            }
        };

        let explicit_decision_step = match arguments.get("decision_step") {
            None => None,
            Some(value) => Some(
                value
                    .as_u64()
                    .ok_or("decision_step must be a non-negative integer")?
                    as usize,
            ),
        };
        let selected_step = match explicit_decision_step {
            Some(step) => match analysis.localise_to(&trace, step) {
                Ok(step) => step,
                Err(error) => {
                    return Ok(json!({
                        "ok": false,
                        "schema": "bioprism-mcp/benchmark-decision-audit/0.1",
                        "stage": "decision_selection",
                        "refusal": error.to_string(),
                        "fail_closed": true,
                        "trace_id": trace.trace_id,
                        "analysis": {
                            "verdict": analysis.verdict,
                            "localized_step": analysis.first_causal_step(),
                        },
                        "guarantees": [
                            "observations and results are never silently replaced by their nearest decision",
                            "an explicit decision step is checked against the actual trace before reconstruction",
                        ],
                    }));
                }
            },
            None => match analysis.first_causal_step() {
                Some(step) => step,
                None => {
                    return Ok(json!({
                        "ok": false,
                        "schema": "bioprism-mcp/benchmark-decision-audit/0.1",
                        "stage": "decision_selection",
                        "refusal": "causal analysis did not localize a decision; provide decision_step only when a reviewer wants an explicit decision audit",
                        "fail_closed": true,
                        "trace_id": trace.trace_id,
                        "analysis": {
                            "verdict": analysis.verdict,
                            "localized_step": analysis.first_causal_step(),
                        },
                        "guarantees": [
                            "environment divergences, no-divergence results, and unlocalizable evidence do not become fabricated cells",
                            "a caller may still request an explicit decision-bearing step for a structural audit",
                        ],
                    }));
                }
            },
        };

        let mut action_set = match BenchmarkCandidateActionSet::reconstruct(&trace, selected_step) {
            Ok(action_set) => action_set,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/benchmark-decision-audit/0.1",
                    "stage": "decision_reconstruction",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "trace_id": trace.trace_id,
                    "decision_step": selected_step,
                    "guarantees": [
                        "only choice and action events can host a candidate action set",
                        "the server does not infer alternatives from a neighboring observation",
                    ],
                }));
            }
        };
        for action in candidate_actions.drain(..) {
            if let Err(error) = action_set.add(action) {
                return Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/benchmark-decision-audit/0.1",
                    "stage": "hindsight_firewall",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "trace_id": trace.trace_id,
                    "decision_step": selected_step,
                    "analysis": {
                        "verdict": analysis.verdict,
                        "localized_step": analysis.first_causal_step(),
                    },
                    "guarantees": [
                        "future-sourced options remain available for validation but cannot be mislabeled as visible at decision time",
                        "a candidate with false visible provenance is rejected before coverage is reported",
                    ],
                }));
            }
        }

        let max_items = max_items as usize;
        let all_actions = action_set.all();
        let visible = action_set.visible_to_agent();
        let validation_only = action_set.validation_only();
        let acceptable = action_set.acceptable();
        let action_total = all_actions.len();
        let visible_total = visible.len();
        let validation_total = validation_only.len();
        let acceptable_total = acceptable.len();
        let actions = all_actions
            .iter()
            .take(max_items)
            .cloned()
            .collect::<Vec<_>>();
        let visible_actions = visible.iter().take(max_items).cloned().collect::<Vec<_>>();
        let validation_actions = validation_only
            .iter()
            .take(max_items)
            .cloned()
            .collect::<Vec<_>>();
        let acceptable_actions = acceptable
            .iter()
            .take(max_items)
            .cloned()
            .collect::<Vec<_>>();

        let card = benchmark_failure_card(&analysis, &constraints, evaluator_dispute, claims);
        let mut card_projection = card.clone();
        let card_omitted = json!({
            "recommended_cell_steps": card.recommended_cell_steps.len().saturating_sub(max_items),
            "findings": card.findings.len().saturating_sub(max_items),
            "hypotheses": card.hypotheses.len().saturating_sub(max_items),
            "violated_constraints": card.violated_constraints.len().saturating_sub(max_items),
            "alternative_explanations": card.alternative_explanations.len().saturating_sub(max_items),
            "missing_evidence": card.missing_evidence.len().saturating_sub(max_items),
        });
        card_projection.recommended_cell_steps.truncate(max_items);
        card_projection.findings.truncate(max_items);
        card_projection.hypotheses.truncate(max_items);
        card_projection.violated_constraints.truncate(max_items);
        card_projection.alternative_explanations.truncate(max_items);
        card_projection.missing_evidence.truncate(max_items);
        let mut card_projection_value = serde_json::to_value(&card_projection)
            .map_err(|error| format!("cannot serialize failure-card projection: {error}"))?;
        if let Some(object) = card_projection_value.as_object_mut() {
            object.insert("evidence_ratio".to_string(), json!(card.evidence_ratio()));
        }

        let ancestry_total = analysis.ancestry.len();
        let candidate_total = analysis.candidates.len();
        let mut analysis_projection = analysis.clone();
        analysis_projection.ancestry.truncate(max_items);
        analysis_projection.candidates.truncate(max_items);

        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/benchmark-decision-audit/0.1",
            "trace_id": trace.trace_id,
            "trace_digest": trace.digest().as_str().to_string(),
            "reference_trace_id": reference.as_ref().map(|trace| trace.trace_id.clone()),
            "reference_digest": reference.as_ref().map(|trace| trace.digest().as_str().to_string()),
            "analysis": analysis_projection,
            "analysis_omitted": {
                "ancestry": ancestry_total.saturating_sub(max_items),
                "candidates": candidate_total.saturating_sub(max_items),
            },
            "decision": {
                "selected_step": selected_step,
                "causal_step": analysis.first_causal_step(),
                "causal_alignment": if analysis.first_causal_step() == Some(selected_step) { "aligned" } else { "explicit_override" },
                "event_kind": trace.at(selected_step).map(|event| event.kind.as_str()),
                "coverage": action_set.coverage(),
                "action_counts": {
                    "all": action_total,
                    "visible_to_agent": visible_total,
                    "validation_only": validation_total,
                    "acceptable": acceptable_total,
                },
                "actions": actions,
                "visible_to_agent": visible_actions,
                "validation_only": validation_actions,
                "acceptable": acceptable_actions,
                "omitted": {
                    "all": action_total.saturating_sub(max_items),
                    "visible_to_agent": visible_total.saturating_sub(max_items),
                    "validation_only": validation_total.saturating_sub(max_items),
                    "acceptable": acceptable_total.saturating_sub(max_items),
                },
            },
            "failure_card": card_projection_value,
            "failure_card_omitted": card_omitted,
            "guarantees": [
                "candidate reconstruction starts from the recorded decision and preserves the hindsight firewall",
                "visible, validation-only, feasible, and strong coverage remain separate evidence counts",
                "causal localization refuses environment-produced divergence instead of moving blame to a nearby action",
                "uncited failure claims remain hypotheses and cannot become evidenced findings",
                "the endpoint produces bounded review material; it does not replay tools, fork an architecture, approve an oracle, or publish a benchmark",
            ],
        }))
    }

    pub(super) fn benchmark_integrity_audit(&self, arguments: &Value) -> Result<Value, String> {
        let raw_instances = arguments
            .get("instances")
            .cloned()
            .ok_or("instances is required and must be an array of benchmark Instance values")?;
        let raw_panel_runs = arguments
            .get("panel_runs")
            .cloned()
            .unwrap_or_else(|| json!([]));
        let raw_bench_instances = arguments
            .get("bench_instances")
            .cloned()
            .unwrap_or_else(|| json!([]));
        let raw_known_instances = arguments
            .get("known_instances")
            .cloned()
            .unwrap_or_else(|| json!([]));
        let raw_safety_vetoes = arguments
            .get("safety_vetoes")
            .cloned()
            .unwrap_or_else(|| json!([]));
        let raw_exposure = arguments
            .get("exposure")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let raw_probes = arguments
            .get("probes")
            .cloned()
            .unwrap_or_else(|| json!({}));

        let max_items = match arguments.get("max_items") {
            None => 100,
            Some(value) => value
                .as_u64()
                .ok_or("max_items must be a non-negative integer")?,
        };
        if max_items == 0 || max_items > 1_000 {
            return Err("max_items must be between 1 and 1000".into());
        }
        let private_share = match arguments.get("private_share") {
            None => 20,
            Some(value) => value
                .as_u64()
                .ok_or("private_share must be a non-negative integer")?,
        };
        if private_share > 100 {
            return Err("private_share must be between 0 and 100".into());
        }
        let rotating_panels = match arguments.get("rotating_panels") {
            None => 0,
            Some(value) => value
                .as_u64()
                .ok_or("rotating_panels must be a non-negative integer")?,
        };
        if rotating_panels > 1_000 {
            return Err("rotating_panels must be between 0 and 1000".into());
        }

        let encoded = serde_json::to_vec(&json!({
            "instances": raw_instances.clone(),
            "panel_runs": raw_panel_runs.clone(),
            "bench_instances": raw_bench_instances.clone(),
            "known_instances": raw_known_instances.clone(),
            "safety_vetoes": raw_safety_vetoes.clone(),
            "exposure": raw_exposure.clone(),
            "probes": raw_probes.clone(),
        }))
        .map_err(|error| format!("cannot measure benchmark integrity envelope: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("benchmark integrity input exceeds the 20000000-byte safety bound".into());
        }

        let instances: Vec<BenchmarkInstance> = serde_json::from_value(raw_instances)
            .map_err(|error| format!("invalid benchmark instance: {error}"))?;
        if instances.len() > 100_000 {
            return Err("instances are bounded at 100000 items".into());
        }
        let mut instance_ids = BTreeSet::new();
        for instance in &instances {
            if !instance_ids.insert(instance.instance_id.clone()) {
                return Err(format!("duplicate instance_id {:?}", instance.instance_id));
            }
        }
        let panel_runs: Vec<BenchmarkPanelRun> = serde_json::from_value(raw_panel_runs)
            .map_err(|error| format!("invalid panel run: {error}"))?;
        if panel_runs.len() > 100_000 {
            return Err("panel_runs are bounded at 100000 items".into());
        }
        let bench_instances: Vec<BenchmarkBenchInstance> =
            serde_json::from_value(raw_bench_instances)
                .map_err(|error| format!("invalid effective-diversity instance: {error}"))?;
        if bench_instances.len() > 100_000 {
            return Err("bench_instances are bounded at 100000 items".into());
        }

        let mut known_instances: BTreeSet<String> = BTreeSet::new();
        let known_array = raw_known_instances
            .as_array()
            .ok_or("known_instances must be an array of strings")?;
        if known_array.len() > 100_000 {
            return Err("known_instances are bounded at 100000 items".into());
        }
        for value in known_array {
            let id = value
                .as_str()
                .ok_or("known_instances must be an array of strings")?;
            known_instances.insert(id.to_string());
        }
        known_instances.extend(instance_ids.iter().cloned());

        let mut safety_vetoes = BTreeSet::new();
        let safety_array = raw_safety_vetoes
            .as_array()
            .ok_or("safety_vetoes must be an array of strings")?;
        if safety_array.len() > 100_000 {
            return Err("safety_vetoes are bounded at 100000 items".into());
        }
        for value in safety_array {
            safety_vetoes.insert(
                value
                    .as_str()
                    .ok_or("safety_vetoes must be an array of strings")?
                    .to_string(),
            );
        }

        let exposure: BTreeMap<String, BenchmarkExposureLedger> =
            serde_json::from_value(raw_exposure)
                .map_err(|error| format!("invalid exposure ledger map: {error}"))?;
        if exposure.len() > 100_000 {
            return Err("exposure is bounded at 100000 instance entries".into());
        }
        let probes: BTreeMap<String, Vec<BenchmarkLeakProbe>> = serde_json::from_value(raw_probes)
            .map_err(|error| format!("invalid contamination probe map: {error}"))?;
        if probes.len() > 100_000 {
            return Err("probes are bounded at 100000 instance entries".into());
        }

        let dedup = benchmark_deduplicate(&instances);
        let dedup_group_total = dedup.groups.len();
        let dedup_groups = dedup
            .groups
            .iter()
            .take(max_items as usize)
            .cloned()
            .collect::<Vec<_>>();
        let removed = dedup.removed();
        let removed_total = removed.len();

        let mut holdout_rows = Vec::with_capacity(instances.len().min(max_items as usize));
        let mut holdout_counts: BTreeMap<String, usize> = BTreeMap::new();
        for instance in &instances {
            let fingerprint = bioprism_benchcompiler::content_fingerprint(instance);
            let holdout =
                benchmark_assign_holdout(instance, private_share as u8, rotating_panels as usize);
            let label = match &holdout {
                bioprism_benchcompiler::Holdout::Public => "public",
                bioprism_benchcompiler::Holdout::Private => "private",
                bioprism_benchcompiler::Holdout::Rotating { .. } => "rotating",
            };
            *holdout_counts.entry(label.to_string()).or_default() += 1;
            if holdout_rows.len() < max_items as usize {
                holdout_rows.push(json!({
                    "instance_id": instance.instance_id,
                    "content_fingerprint": fingerprint,
                    "holdout": holdout,
                }));
            }
        }

        let mut contamination_rows = Vec::with_capacity(instances.len().min(max_items as usize));
        let mut contamination_counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut admissible = 0usize;
        for instance in &instances {
            let ledger = exposure
                .get(&instance.instance_id)
                .cloned()
                .unwrap_or_default();
            let ledger_provided = exposure.contains_key(&instance.instance_id);
            let probe_rows = probes
                .get(&instance.instance_id)
                .cloned()
                .unwrap_or_default();
            let report = benchmark_assess_contamination(instance, &ledger, &probe_rows);
            let risk_label = match &report.risk {
                BenchmarkContaminationRisk::LeaksThroughChannel { .. } => "leaks_through_channel",
                BenchmarkContaminationRisk::AnswerSearchable { .. } => "answer_searchable",
                BenchmarkContaminationRisk::PublishedAndUnprobed => "published_and_unprobed",
                BenchmarkContaminationRisk::Unassessed => "unassessed",
                BenchmarkContaminationRisk::Clean => "clean",
            };
            *contamination_counts
                .entry(risk_label.to_string())
                .or_default() += 1;
            if report.risk.admissible() {
                admissible += 1;
            }
            if contamination_rows.len() < max_items as usize {
                contamination_rows.push(json!({
                    "instance_id": instance.instance_id,
                    "ledger_provided": ledger_provided,
                    "report": report,
                    "admissible": report.risk.admissible(),
                }));
            }
        }

        let calibration = benchmark_calibrate(&panel_runs, &known_instances, &safety_vetoes);
        let calibration_total = calibration.instances.len();
        let calibration_instances = calibration
            .instances
            .iter()
            .take(max_items as usize)
            .cloned()
            .collect::<Vec<_>>();
        let diversity = benchmark_effective_diversity(&bench_instances);
        let instance_digest = bioprism_ids::ContentHash::of_value(
            &serde_json::to_value(&instances)
                .map_err(|error| format!("cannot serialize instance digest input: {error}"))?,
        )
        .map_err(|error| format!("cannot hash benchmark instances: {error}"))?
        .as_str()
        .to_string();

        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/benchmark-integrity-audit/0.1",
            "instance_digest": instance_digest,
            "counts": {
                "instances": instances.len(),
                "panel_runs": panel_runs.len(),
                "bench_instances": bench_instances.len(),
                "known_instances": known_instances.len(),
                "safety_vetoes": safety_vetoes.len(),
            },
            "dedup": {
                "examined": dedup.examined,
                "distinct": dedup.distinct,
                "groups": dedup_groups,
                "groups_omitted": dedup_group_total.saturating_sub(max_items as usize),
                "removed": removed.iter().take(max_items as usize).collect::<Vec<_>>(),
                "removed_omitted": removed_total.saturating_sub(max_items as usize),
                "caveat": dedup.caveat,
            },
            "holdout": {
                "private_share": private_share,
                "rotating_panels": rotating_panels,
                "counts": holdout_counts,
                "rows": holdout_rows,
                "omitted": instances.len().saturating_sub(max_items as usize),
            },
            "contamination": {
                "counts": contamination_counts,
                "admissible": admissible,
                "inadmissible": instances.len().saturating_sub(admissible),
                "rows": contamination_rows,
                "omitted": instances.len().saturating_sub(max_items as usize),
            },
            "calibration": {
                "discriminating": calibration.discriminating,
                "trivial_cue": calibration.trivial_cue,
                "universally_passed": calibration.universally_passed,
                "universally_failed": calibration.universally_failed,
                "unmeasured": calibration.unmeasured,
                "safety_vetoes": calibration.safety_vetoes,
                "instances": calibration_instances,
                "omitted": calibration_total.saturating_sub(max_items as usize),
            },
            "effective_diversity": diversity,
            "guarantees": [
                "content and structural deduplication exclude labels from exact fingerprints and report oracle-equivalent groups without silently deleting them",
                "holdout assignment is deterministic from content rather than random state, so a second site can reproduce the split",
                "contamination is assessed in declared severity order; a missing ledger is unassessed, never clean",
                "unmeasured calibration is distinct from universal failure, and safety vetoes remain labelled rather than pruned",
                "effective diversity counts independent (parent, mutation family, oracle signature) classes rather than raw instance volume",
                "bounded projections carry omitted counts and this endpoint does not discover publication, run leak probes, fit difficulty models, or compute semantic similarity",
            ],
        }))
    }

    pub(super) fn benchmark_counterfactual_check(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let raw_source = arguments
            .get("source")
            .cloned()
            .ok_or("source is required and must be a serialized DecisionCell")?;
        let raw_followup = arguments
            .get("followup")
            .cloned()
            .ok_or("followup is required and must be a serialized DecisionCell")?;
        let raw_intervention = arguments
            .get("intervention")
            .cloned()
            .ok_or("intervention is required and must be a serialized Intervention")?;
        let raw_expected = arguments
            .get("expected")
            .cloned()
            .ok_or("expected is required and must be an ExpectedResponse")?;
        let source_verdict = arguments
            .get("source_verdict")
            .and_then(Value::as_str)
            .ok_or("source_verdict is required and must be a string")?;
        let followup_verdict = arguments
            .get("followup_verdict")
            .and_then(Value::as_str)
            .ok_or("followup_verdict is required and must be a string")?;
        if source_verdict.is_empty() || followup_verdict.is_empty() {
            return Err("source_verdict and followup_verdict must not be empty".into());
        }

        let encoded = serde_json::to_vec(&json!({
            "source": raw_source.clone(),
            "followup": raw_followup.clone(),
            "intervention": raw_intervention.clone(),
            "expected": raw_expected.clone(),
            "source_verdict": source_verdict,
            "followup_verdict": followup_verdict,
        }))
        .map_err(|error| format!("cannot measure counterfactual input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("counterfactual input exceeds the 20000000-byte safety bound".into());
        }

        let source: bioprism_prism::DecisionCell = serde_json::from_value(raw_source)
            .map_err(|error| format!("invalid source DecisionCell: {error}"))?;
        let followup: bioprism_prism::DecisionCell = serde_json::from_value(raw_followup)
            .map_err(|error| format!("invalid followup DecisionCell: {error}"))?;
        let intervention: BenchmarkIntervention = serde_json::from_value(raw_intervention)
            .map_err(|error| format!("invalid Intervention: {error}"))?;
        let expected: BenchmarkExpectedResponse = serde_json::from_value(raw_expected)
            .map_err(|error| format!("invalid ExpectedResponse: {error}"))?;
        let mut no_realism_review = BenchmarkNoRealismReview;
        let pair = match benchmark_counterfactual_pair(
            source,
            followup,
            intervention,
            expected,
            &mut no_realism_review,
            false,
        ) {
            Ok(pair) => pair,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/benchmark-counterfactual/0.1",
                    "stage": "matched_pair",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "allowed_cell_fields": bioprism_benchcompiler::CELL_FIELDS,
                    "guarantees": [
                        "a pair is refused unless the source and follow-up have distinct ids and a non-null intervention",
                        "every changed cell field must be declared by the intervention",
                        "absence of a runtime/domain realism validator is represented as an explicit limitation, never as a realism pass",
                    ],
                }));
            }
        };
        let outcome = benchmark_contrast(&pair, source_verdict, followup_verdict);
        let satisfied = matches!(
            &outcome,
            bioprism_benchcompiler::ContrastOutcome::AsPredicted
        );
        let source_digest = pair.source.digest().as_str().to_string();
        let followup_digest = pair.followup.digest().as_str().to_string();
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/benchmark-counterfactual/0.1",
            "pair": pair,
            "outcome": outcome,
            "satisfied": satisfied,
            "source_verdict": source_verdict,
            "followup_verdict": followup_verdict,
            "cell_digests": {
                "source": source_digest,
                "followup": followup_digest,
            },
            "allowed_cell_fields": bioprism_benchcompiler::CELL_FIELDS,
            "guarantees": [
                "the pair differs only in the intervention-declared fields",
                "contrast grades the candidate response against the declared invariant or must-change expectation",
                "source and follow-up cell contracts remain set-valued and digest-bound",
            ],
            "limitations": [
                "this endpoint validates and contrasts caller-constructed cells; it does not apply an intervention or execute a world",
                "realism_reviewed is false because the MCP boundary has no domain validator; caller-side realism evidence remains required",
            ],
        }))
    }

    pub(super) fn benchmark_oracle_review(&self, arguments: &Value) -> Result<Value, String> {
        let raw_proposal = arguments
            .get("proposal")
            .cloned()
            .ok_or("proposal is required and must be a serialized ProposedOracle")?;
        let reviewer = arguments
            .get("reviewer")
            .and_then(Value::as_str)
            .ok_or("reviewer is required and must be a string")?;
        let encoded = serde_json::to_vec(&json!({
            "proposal": raw_proposal.clone(),
            "reviewer": reviewer,
            "grade": arguments.get("grade"),
            "cell": arguments.get("cell"),
        }))
        .map_err(|error| format!("cannot measure oracle review envelope: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("oracle review input exceeds the 20000000-byte safety bound".into());
        }

        let proposal: BenchmarkProposedOracle = serde_json::from_value(raw_proposal.clone())
            .map_err(|error| format!("invalid ProposedOracle: {error}"))?;
        let proposal_value = serde_json::to_value(&proposal)
            .map_err(|error| format!("cannot serialize ProposedOracle: {error}"))?;
        let reviewed = match proposal.review(reviewer) {
            Ok(reviewed) => reviewed,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/benchmark-oracle-review/0.1",
                    "stage": "oracle_review",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "proposal": proposal_value,
                    "reviewer": reviewer,
                    "synthesis_order": BENCHMARK_SYNTHESIS_ORDER.iter().map(|strength| strength.as_str()).collect::<Vec<_>>(),
                    "guarantees": [
                        "an unreviewed proposal cannot grade or package a DecisionCell",
                        "unattributed review, empty acceptance, missing gap analysis, successful exploits, and weak-oracle-alone proposals remain blocking findings",
                    ],
                }));
            }
        };

        let proposal = reviewed.proposal();
        let grade = match arguments.get("grade") {
            None => Value::Null,
            Some(raw_grade) => {
                let grade = raw_grade.as_object().ok_or("grade must be an object")?;
                let verdict = grade
                    .get("verdict")
                    .and_then(Value::as_str)
                    .ok_or("grade.verdict must be a string")?;
                let witnesses = grade
                    .get("witnesses")
                    .and_then(Value::as_array)
                    .ok_or("grade.witnesses must be an array of strings")?;
                let mut witness_set = BTreeSet::new();
                for witness in witnesses {
                    witness_set.insert(
                        witness
                            .as_str()
                            .ok_or("grade.witnesses must be an array of strings")?
                            .to_string(),
                    );
                }
                let closure_complete = grade
                    .get("closure_complete")
                    .and_then(Value::as_bool)
                    .ok_or("grade.closure_complete must be a boolean")?;
                let acceptance = reviewed.grade(verdict, &witness_set, closure_complete);
                let passed = acceptance.passed();
                let reason = acceptance.reason();
                json!({
                    "verdict": verdict,
                    "witnesses": witness_set,
                    "closure_complete": closure_complete,
                    "acceptance": acceptance,
                    "passed": passed,
                    "reason": reason,
                })
            }
        };

        let cell = match arguments.get("cell") {
            None => Value::Null,
            Some(raw_cell) => {
                let cell = raw_cell.as_object().ok_or("cell must be an object")?;
                let cell_id = cell
                    .get("cell_id")
                    .and_then(Value::as_str)
                    .ok_or("cell.cell_id must be a string")?;
                let world: bioprism_prism::InputRef = serde_json::from_value(
                    cell.get("world").cloned().ok_or("cell.world is required")?,
                )
                .map_err(|error| format!("invalid cell.world InputRef: {error}"))?;
                let query: bioprism_prism::InputRef = serde_json::from_value(
                    cell.get("query").cloned().ok_or("cell.query is required")?,
                )
                .map_err(|error| format!("invalid cell.query InputRef: {error}"))?;
                serde_json::to_value(reviewed.clone().into_cell(cell_id, world, query))
                    .map_err(|error| format!("cannot serialize reviewed DecisionCell: {error}"))?
            }
        };

        let reviewed_value = serde_json::to_value(&reviewed)
            .map_err(|error| format!("cannot serialize ReviewedOracle: {error}"))?;
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/benchmark-oracle-review/0.1",
            "proposal": proposal,
            "reviewed_oracle": reviewed_value,
            "reviewer": reviewed.reviewer(),
            "review_digest": reviewed.review_digest(),
            "strength": proposal.strength,
            "deterministic": proposal.strength.deterministic(),
            "grade": grade,
            "cell": cell,
            "synthesis_order": BENCHMARK_SYNTHESIS_ORDER.iter().map(|strength| strength.as_str()).collect::<Vec<_>>(),
            "guarantees": [
                "only the kernel review gate creates a ReviewedOracle; serialized reviewed output is not accepted as trusted input",
                "grading preserves wrong-verdict, missing-witness, closure-incomplete, and passed outcomes",
                "cell packaging copies the reviewed set-valued verdict and witness contract",
            ],
            "limitations": [
                "oracle contracts are declarative; this endpoint does not execute checker code, run attacks, or validate an external world",
                "a model judge or statistical tolerance remains non-deterministic and cannot stand alone without a paired deterministic oracle",
            ],
        }))
    }

    pub(super) fn benchmark_compile(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot measure benchmark compiler input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("benchmark compiler input exceeds the 20000000-byte safety bound".into());
        }
        let failing: TraceIr = serde_json::from_value(
            arguments
                .get("trace")
                .cloned()
                .ok_or("trace is required and must be a serialized bioprism_trace Trace")?,
        )
        .map_err(|error| format!("invalid benchmark failing trace: {error}"))?;
        if failing.events.len() > 100_000 {
            return Err("trace may contain at most 100000 events".into());
        }
        let reference: Option<TraceIr> = match arguments.get("reference") {
            None | Some(Value::Null) => None,
            Some(value) => Some(
                serde_json::from_value(value.clone())
                    .map_err(|error| format!("invalid benchmark reference trace: {error}"))?,
            ),
        };
        if reference
            .as_ref()
            .is_some_and(|trace| trace.events.len() > 100_000)
        {
            return Err("reference trace may contain at most 100000 events".into());
        }
        let context: Vec<BenchmarkContextItem> = serde_json::from_value(
            arguments
                .get("context")
                .cloned()
                .unwrap_or_else(|| json!([])),
        )
        .map_err(|error| format!("invalid benchmark context: {error}"))?;
        if context.len() > 5_000 {
            return Err("context may contain at most 5000 items".into());
        }
        let mut context_ids = BTreeSet::new();
        for item in &context {
            if !context_ids.insert(item.id.clone()) {
                return Err(format!("context contains duplicate item id {}", item.id));
            }
        }

        let observation_values = arguments
            .get("probe_observations")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if observation_values.len() > 100_000 {
            return Err("probe_observations may contain at most 100000 rows".into());
        }
        let mut observations = BTreeMap::<String, BenchmarkInterestSignature>::new();
        for row in observation_values {
            let row = row
                .as_object()
                .ok_or("each probe_observations row must be an object")?;
            let kept = row
                .get("kept")
                .and_then(Value::as_array)
                .ok_or("each probe_observations row requires kept as an array")?;
            let mut ids = BTreeSet::new();
            for id in kept {
                ids.insert(
                    id.as_str()
                        .ok_or("probe_observations.kept must contain strings")?
                        .to_string(),
                );
            }
            if ids.iter().any(|id| !context_ids.contains(id)) {
                return Err("probe_observations.kept names an id outside context".into());
            }
            let signature: BenchmarkInterestSignature = serde_json::from_value(
                row.get("signature")
                    .cloned()
                    .ok_or("each probe_observations row requires signature")?,
            )
            .map_err(|error| format!("invalid probe observation signature: {error}"))?;
            let key = ids.iter().cloned().collect::<Vec<_>>().join("\u{1f}");
            if observations.insert(key, signature).is_some() {
                return Err("probe_observations contains duplicate kept subsets".into());
            }
        }
        if !context.is_empty() && arguments.get("probe_observations").is_none() {
            return Ok(json!({
                "ok": false,
                "schema": "bioprism-mcp/benchmark-compile/0.1",
                "stage": "minimization_probe",
                "refusal": "context is non-empty but no caller-supplied probe observations were provided",
                "fail_closed": true,
                "guarantees": [
                    "the compiler never invents an interest signature or silently executes a caller probe",
                    "a minimization without an observable preservation contract cannot produce an oracle proposal",
                ],
            }));
        }
        let budget_value = arguments
            .get("budget")
            .cloned()
            .unwrap_or_else(|| json!({"max_evaluations": 4096}));
        let budget: BenchmarkMinimizeBudget = serde_json::from_value(budget_value)
            .map_err(|error| format!("invalid benchmark minimization budget: {error}"))?;
        if budget.max_evaluations == 0 || budget.max_evaluations > 100_000 {
            return Err("budget.max_evaluations must be between 1 and 100000".into());
        }
        let ledger: Vec<BenchmarkConstraintRecord> = serde_json::from_value(
            arguments
                .get("ledger")
                .cloned()
                .unwrap_or_else(|| json!([])),
        )
        .map_err(|error| format!("invalid benchmark constraint ledger: {error}"))?;
        if ledger.len() > 10_000 {
            return Err("ledger may contain at most 10000 records".into());
        }
        let claims: Vec<BenchmarkAssertion> = serde_json::from_value(
            arguments
                .get("claims")
                .cloned()
                .unwrap_or_else(|| json!([])),
        )
        .map_err(|error| format!("invalid benchmark claims: {error}"))?;
        if claims.len() > 10_000 {
            return Err("claims may contain at most 10000 records".into());
        }
        if claims.iter().any(|claim| {
            matches!(
                claim,
                BenchmarkAssertion::Evidenced { citations, .. } if citations.is_empty()
            )
        }) {
            return Ok(json!({
                "ok": false,
                "schema": "bioprism-mcp/benchmark-compile/0.1",
                "stage": "claim_attribution",
                "trace_id": failing.trace_id,
                "refusal": "an evidenced benchmark claim must cite an event, artifact, state diff, or counterfactual outcome",
                "fail_closed": true,
                "guarantees": [
                    "uncited claims remain hypotheses and cannot enter backed failure findings",
                    "the compiler does not repair or silently downgrade a caller's malformed evidence claim",
                ],
            }));
        }

        let mut missing_observations = BTreeSet::new();
        let mut probe = |kept: &BTreeSet<String>| -> BenchmarkInterestSignature {
            let key = kept.iter().cloned().collect::<Vec<_>>().join("\u{1f}");
            match observations.get(&key) {
                Some(signature) => signature.clone(),
                None => {
                    missing_observations.insert(key);
                    BenchmarkInterestSignature::new("__missing_probe_observation__")
                }
            }
        };
        let compilation = match benchmark_compile(
            &failing,
            reference.as_ref(),
            &context,
            &mut probe,
            budget,
            &ledger,
            claims,
        ) {
            Ok(compilation) => compilation,
            Err(error) => {
                if !missing_observations.is_empty() {
                    return Ok(json!({
                        "ok": false,
                        "schema": "bioprism-mcp/benchmark-compile/0.1",
                        "stage": "minimization_probe",
                        "trace_id": failing.trace_id,
                        "refusal": "the caller-supplied probe observation table did not cover every subset requested by deterministic minimization",
                        "compiler_error": error.to_string(),
                        "fail_closed": true,
                        "provided_rows": observations.len(),
                        "missing_rows": missing_observations.len(),
                        "missing_subsets": missing_observations.iter().take(100).cloned().collect::<Vec<_>>(),
                        "missing_subsets_omitted": missing_observations.len().saturating_sub(100),
                        "guarantees": [
                            "a partial observation table cannot be interpolated into a preservation proof",
                            "no proposed oracle is returned when minimization evidence is incomplete",
                        ],
                    }));
                }
                return Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/benchmark-compile/0.1",
                    "stage": "benchmark_compile",
                    "trace_id": failing.trace_id,
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "missing_probe_rows": missing_observations.len(),
                    "guarantees": [
                        "causal, minimization, and oracle errors remain typed compiler refusals",
                        "no rejected compilation is presented as a DecisionCell or benchmark score",
                    ],
                }));
            }
        };
        if !missing_observations.is_empty() {
            return Ok(json!({
                "ok": false,
                "schema": "bioprism-mcp/benchmark-compile/0.1",
                "stage": "minimization_probe",
                "trace_id": failing.trace_id,
                "refusal": "the caller-supplied probe observation table did not cover every subset requested by deterministic minimization",
                "fail_closed": true,
                "provided_rows": observations.len(),
                "missing_rows": missing_observations.len(),
                "missing_subsets": missing_observations.iter().take(100).cloned().collect::<Vec<_>>(),
                "missing_subsets_omitted": missing_observations.len().saturating_sub(100),
                "guarantees": [
                    "a partial observation table cannot be interpolated into a preservation proof",
                    "no proposed oracle is returned when minimization evidence is incomplete",
                ],
            }));
        }
        let compilation_value = serde_json::to_value(&compilation)
            .map_err(|error| format!("cannot serialize benchmark compilation: {error}"))?;
        let minimization = compilation.minimization.as_ref().map(|result| {
            json!({
                "started_from": result.started_from,
                "minimal": result.minimal,
                "removed": result.removed,
                "pinned": result.pinned,
                "preserved": result.preserved,
                "evaluations": result.evaluations,
                "passes": result.passes,
                "reduction_ratio": result.reduction_ratio(),
                "minimality_witness_count": result.minimality_witnesses.len(),
                "guarantee": result.guarantee,
            })
        });
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/benchmark-compile/0.1",
            "trace_id": compilation.trace_id,
            "trace_digest": failing.digest().to_string(),
            "reference_digest": reference.as_ref().map(|trace| trace.digest().to_string()),
            "compilation": compilation_value,
            "class": compilation.class,
            "cell_step": compilation.cell_step(),
            "episodes": compilation.episodes,
            "boundary_count": compilation.boundaries.len(),
            "oracle": compilation.oracle,
            "minimization": minimization,
            "confidence": compilation.confidence,
            "limiting_stage": compilation.confidence.limiting_stage(),
            "unmeasured_stages": compilation.confidence.unmeasured_stages(),
            "probe": {
                "provided_rows": observations.len(),
                "evaluations": compilation.minimization.as_ref().map(|result| result.evaluations).unwrap_or(0),
                "execution": "caller-supplied observation table; no world or architecture was run",
            },
            "guarantees": [
                "causal localization, reduction, oracle synthesis, and decomposed confidence are returned from the typed benchcompiler pipeline",
                "the output stops at an unreviewed ProposedOracle; only benchmark_oracle_review can create a ReviewedOracle",
                "unmeasured pipeline stages remain explicit and are never collapsed into an aggregate score",
            ],
            "limitations": [
                "the MCP boundary replays no world and accepts no executable probe; callers must provide an exact observation row for every subset requested",
                "the pipeline does not generate mutations, run exploit attacks, validate realism, or publish a benchmark pack",
            ],
        }))
    }

    pub(super) fn benchmark_compile_review(&self, arguments: &Value) -> Result<Value, String> {
        let reviewer = arguments
            .get("reviewer")
            .and_then(Value::as_str)
            .ok_or("reviewer is required and must be a string")?;
        let world: bioprism_prism::InputRef = serde_json::from_value(
            arguments
                .get("world")
                .cloned()
                .ok_or("world is required and must be a serialized InputRef")?,
        )
        .map_err(|error| format!("invalid reviewed-cell world InputRef: {error}"))?;
        let query: bioprism_prism::InputRef = serde_json::from_value(
            arguments
                .get("query")
                .cloned()
                .ok_or("query is required and must be a serialized InputRef")?,
        )
        .map_err(|error| format!("invalid reviewed-cell query InputRef: {error}"))?;
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot measure compile-review input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err(
                "benchmark compile-review input exceeds the 20000000-byte safety bound".into(),
            );
        }
        let compiled = self.benchmark_compile(arguments)?;
        if compiled.get("ok") != Some(&Value::Bool(true)) {
            return Ok(json!({
                "ok": false,
                "schema": "bioprism-mcp/benchmark-compile-review/0.1",
                "stage": "benchmark_compile",
                "compile": compiled,
                "refusal": "benchmark compilation refused before the review gate",
                "fail_closed": true,
                "guarantees": [
                    "a failed compilation cannot be promoted into a reviewed cell",
                    "compiler-stage refusals and review-stage refusals remain distinguishable",
                ],
            }));
        }
        let compilation: BenchmarkCompilation = serde_json::from_value(
            compiled
                .get("compilation")
                .cloned()
                .ok_or("benchmark compilation returned no Compilation projection")?,
        )
        .map_err(|error| format!("invalid internal benchmark Compilation projection: {error}"))?;
        let (cell, reviewed) = match compilation.approve(reviewer, world, query) {
            Ok(result) => result,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "schema": "bioprism-mcp/benchmark-compile-review/0.1",
                    "stage": "oracle_review",
                    "compile": compiled,
                    "reviewer": reviewer,
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "guarantees": [
                        "a compilation without a causal cell and reviewed oracle cannot be packaged",
                        "unattributed, weak, exploited, or gap-free oracle proposals remain blocking",
                    ],
                }));
            }
        };
        let grade = match arguments.get("grade") {
            None => Value::Null,
            Some(raw_grade) => {
                let grade = raw_grade.as_object().ok_or("grade must be an object")?;
                let verdict = grade
                    .get("verdict")
                    .and_then(Value::as_str)
                    .ok_or("grade.verdict must be a string")?;
                let witnesses = grade
                    .get("witnesses")
                    .and_then(Value::as_array)
                    .ok_or("grade.witnesses must be an array of strings")?;
                let mut witness_set = BTreeSet::new();
                for witness in witnesses {
                    witness_set.insert(
                        witness
                            .as_str()
                            .ok_or("grade.witnesses must be an array of strings")?
                            .to_string(),
                    );
                }
                let closure_complete = grade
                    .get("closure_complete")
                    .and_then(Value::as_bool)
                    .ok_or("grade.closure_complete must be a boolean")?;
                let acceptance = reviewed.grade(verdict, &witness_set, closure_complete);
                json!({
                    "verdict": verdict,
                    "witnesses": witness_set,
                    "closure_complete": closure_complete,
                    "acceptance": acceptance,
                    "passed": acceptance.passed(),
                    "reason": acceptance.reason(),
                })
            }
        };
        let reviewed_value = serde_json::to_value(&reviewed)
            .map_err(|error| format!("cannot serialize reviewed oracle: {error}"))?;
        let cell_value = serde_json::to_value(&cell)
            .map_err(|error| format!("cannot serialize reviewed DecisionCell: {error}"))?;
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/benchmark-compile-review/0.1",
            "compile": compiled,
            "reviewed_oracle": reviewed_value,
            "reviewer": reviewed.reviewer(),
            "review_digest": reviewed.review_digest(),
            "grade": grade,
            "cell": cell_value,
            "guarantees": [
                "the assembled compiler output is reviewed before a DecisionCell is packaged",
                "the cell carries the reviewed set-valued verdict and witness contract",
                "the optional grade preserves wrong-verdict, missing-witness, closure-incomplete, and passed outcomes",
            ],
            "limitations": [
                "the compiler still uses caller-supplied probe observations and executes no world, model, or architecture",
                "review does not run exploit generation, realism validation, mutation calibration, or pack publication",
            ],
        }))
    }

    pub(super) fn stress_profile(&self, arguments: &Value) -> Result<Value, String> {
        let cohort: Cohort = serde_json::from_value(
            arguments
                .get("cohort")
                .cloned()
                .ok_or("cohort is required and must be a serialized stress Cohort")?,
        )
        .map_err(|error| format!("invalid stress cohort: {error}"))?;
        if cohort.subjects.len() > 10_000 {
            return Err("cohort exceeds the 10000-subject safety bound".into());
        }
        let stress: Stress = serde_json::from_value(
            arguments
                .get("stress")
                .cloned()
                .ok_or("stress is required and must be a serialized Stress")?,
        )
        .map_err(|error| format!("invalid stress: {error}"))?;
        let procedures: Vec<Procedure> = match arguments.get("procedures") {
            None => standard_panel(),
            Some(value) => serde_json::from_value(value.clone())
                .map_err(|error| format!("invalid stress procedures: {error}"))?,
        };
        if procedures.len() > 100 {
            return Err("stress procedure panel exceeds the 100-procedure safety bound".into());
        }
        match stress_profile_run(&cohort, &stress, &procedures) {
            Ok(profile) => Ok(json!({
                "ok": true,
                "headline": profile.headline(),
                "profile": profile,
                "guarantees": [
                    "breaking points are reported on the declared intensity ladder rather than collapsed into a survival score",
                    "generator defects and batch confounding suppress robustness claims instead of being scored as findings",
                    "effective sample size, unresolved subjects, and analysable prevalence remain visible at every rung"
                ],
                "limitations": [
                    "procedures are closed-form reference summaries; no model is fitted",
                    "the profile is conditional on this cohort, stress seed, family, and finite intensity ladder"
                ]
            })),
            Err(error) => Ok(json!({
                "ok": false,
                "stage": "stress_profile",
                "refusal": error.to_string(),
                "fail_closed": true,
                "guarantee": "invalid cohorts, undefined conclusions, confounded stresses, and broken postconditions never become a robustness claim"
            })),
        }
    }

    pub(super) fn stress_report(&self, arguments: &Value) -> Result<Value, String> {
        let cohort: Cohort = serde_json::from_value(
            arguments
                .get("cohort")
                .cloned()
                .ok_or("cohort is required and must be a serialized stress Cohort")?,
        )
        .map_err(|error| format!("invalid stress cohort: {error}"))?;
        if cohort.subjects.len() > 10_000 {
            return Err("cohort exceeds the 10000-subject safety bound".into());
        }
        let raw_stresses = arguments
            .get("stresses")
            .cloned()
            .ok_or("stresses is required and must be an array of Stress values")?;
        let stresses: Vec<Stress> = serde_json::from_value(raw_stresses)
            .map_err(|error| format!("invalid stress list: {error}"))?;
        if stresses.len() > 100 {
            return Err("stress program exceeds the 100-stress safety bound".into());
        }
        let procedures: Vec<Procedure> = match arguments.get("procedures") {
            None => standard_panel(),
            Some(value) => serde_json::from_value(value.clone())
                .map_err(|error| format!("invalid stress procedures: {error}"))?,
        };
        if procedures.len() > 100 {
            return Err("stress procedure panel exceeds the 100-procedure safety bound".into());
        }
        match StressReport::run(&cohort, &stresses, &procedures) {
            Ok(report) => Ok(json!({
                "ok": true,
                "headline": report.headline(),
                "worst_family": report.worst_family(),
                "report": report,
                "guarantees": [
                    "each family remains a separate robustness profile",
                    "the worst-family view excludes non-identifiable and generator-defective profiles",
                    "a report never turns a required relation failure into a fragile biological conclusion"
                ],
                "limitations": [
                    "family magnitudes are comparable only as declared endpoint fractions, not as physical units",
                    "no causal graph, preanalytic lifecycle, or ontology-drift stress is inferred beyond the supplied program"
                ]
            })),
            Err(error) => Ok(json!({
                "ok": false,
                "stage": "stress_report",
                "refusal": error.to_string(),
                "fail_closed": true,
                "guarantee": "a partial stress program never becomes an apparently complete report"
            })),
        }
    }
}
