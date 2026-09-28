//! Pack, atlas, publication, and bundle release assurance handlers.

use super::*;

impl Server {
    pub(super) fn pack_catalogue(&self, arguments: &Value) -> Result<Value, String> {
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if max_items == 0 || max_items > 1_000 {
            return Err("max_items must be between 1 and 1000".into());
        }
        let section = arguments
            .get("section")
            .and_then(Value::as_str)
            .unwrap_or("all");
        if !matches!(section, "all" | "15" | "29") {
            return Err("section must be one of all, 15, or 29".into());
        }
        let selected = all_packs()
            .iter()
            .filter(|pack| {
                section == "all"
                    || (section == "15" && pack.blueprint_module.starts_with("15."))
                    || (section == "29" && pack.blueprint_module.starts_with("29."))
            })
            .collect::<Vec<_>>();
        let portfolio_count = all_packs().len();
        let returned = selected
            .iter()
            .take(max_items as usize)
            .map(|pack| {
                json!({
                    "id": pack.id,
                    "title": pack.title,
                    "blueprint_module": pack.blueprint_module,
                    "axis": pack.axis,
                    "measures": pack.measures,
                    "capabilities": pack.capabilities.iter().map(|capability| capability.code()).collect::<Vec<_>>(),
                    "domains": pack.domains.iter().map(|domain| domain.label()).collect::<Vec<_>>(),
                    "decision_families": pack.decision_families,
                    "oracles": pack.oracles,
                    "strongest_oracle": pack.strongest_oracle(),
                    "has_execution_grounded_oracle": pack.has_grounded_oracle(),
                    "release_wave": pack.release_wave,
                    "capability_signature": pack.capability_signature(),
                })
            })
            .collect::<Vec<_>>();
        let section_15_count = all_packs()
            .iter()
            .filter(|pack| pack.blueprint_module.starts_with("15."))
            .count();
        let section_29_count = all_packs()
            .iter()
            .filter(|pack| pack.blueprint_module.starts_with("29."))
            .count();
        let duplicate_signatures = duplicate_pack_signatures()
            .into_iter()
            .map(|(signature, ids)| json!({ "signature": signature, "pack_ids": ids }))
            .collect::<Vec<_>>();
        Ok(json!({
            "ok": true,
            "section": section,
            "portfolio_count": portfolio_count,
            "section_counts": { "15": section_15_count, "29": section_29_count },
            "returned": returned,
            "omitted": selected.len().saturating_sub(max_items as usize),
            "duplicate_signature_groups": duplicate_signatures,
            "guarantees": [
                "catalogue rows are typed portfolio declarations, not measured system scores",
                "oracle tier is an upper bound on how disagreement could be decided and is not reliability evidence",
                "release order is reported only where the blueprint declared one; unsequenced packs remain unsequenced",
                "duplicate signatures are review candidates and are not silently deduplicated",
            ],
        }))
    }

    pub(super) fn bioatlas_publication_audit(&self, arguments: &Value) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot measure BioAtlas publication input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("BioAtlas publication input exceeds the 20000000-byte safety bound".into());
        }

        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if !(1..=1_000).contains(&max_items) {
            return Err("max_items must be between 1 and 1000".into());
        }
        let max_items = max_items as usize;

        let raw_atlas = arguments
            .get("atlas")
            .cloned()
            .ok_or("atlas is required and must be a serialized bioprism-atlas Atlas")?;
        let mut atlas_arguments = serde_json::Map::new();
        atlas_arguments.insert("atlas".into(), raw_atlas);
        if let Some(weighting) = arguments.get("weighting") {
            atlas_arguments.insert("weighting".into(), weighting.clone());
        }
        atlas_arguments.insert("max_items".into(), json!(max_items));
        let atlas = self.atlas_report(&Value::Object(atlas_arguments))?;

        let evidence_audit = match arguments.get("evidence_audit") {
            Some(raw) => Some(self.biocapability_evidence_audit(raw)?),
            None => None,
        };
        let card = match arguments.get("card") {
            Some(raw) => Some(self.hub_card_render(raw)?),
            None => None,
        };
        let leaderboard = match arguments.get("leaderboard") {
            Some(raw) => Some(self.hub_leaderboard_render(raw)?),
            None => None,
        };

        let atlas_ok = atlas.get("ok").and_then(Value::as_bool).unwrap_or(false);
        let atlas_aggregation_ready = atlas
            .get("summary")
            .and_then(Value::as_object)
            .and_then(|summary| summary.get("coverage_supports_aggregation"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let evidence_ready = evidence_audit
            .as_ref()
            .and_then(|value| value.get("release_posture"))
            .and_then(|posture| posture.get("ready_for_requested_claims"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let card_ok = card
            .as_ref()
            .and_then(|value| value.get("ok"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let card_score_attached = card
            .as_ref()
            .and_then(|value| value.get("score"))
            .and_then(|score| score.get("attached"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let leaderboard_ok = leaderboard
            .as_ref()
            .and_then(|value| value.get("ok"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let ranked_count = leaderboard
            .as_ref()
            .and_then(|value| value.get("ranked_count"))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let unranked_count = leaderboard
            .as_ref()
            .and_then(|value| value.get("unranked_count"))
            .and_then(Value::as_u64)
            .unwrap_or(0);

        let release_request = if let Some(raw_request) = arguments.get("release_request") {
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
                let (eligible, blockers, notes): (bool, Vec<&str>, Vec<&str>) = match target {
                    "atlas_profile" => (
                        atlas_ok,
                        if atlas_ok {
                            vec![]
                        } else {
                            vec!["atlas_audit_failed"]
                        },
                        vec!["atlas coverage and holes remain visible in the atlas result"],
                    ),
                    "atlas_aggregation" => (
                        atlas_ok && atlas_aggregation_ready,
                        if !atlas_ok {
                            vec!["atlas_audit_failed"]
                        } else if !atlas_aggregation_ready {
                            vec!["coverage_does_not_support_aggregation"]
                        } else {
                            vec![]
                        },
                        vec!["aggregation readiness is derived from the atlas coverage gate"],
                    ),
                    "evidence_claims" => (
                        evidence_ready,
                        if evidence_audit.is_none() {
                            vec!["evidence_audit_missing"]
                        } else if !evidence_ready {
                            vec!["evidence_claims_not_ready"]
                        } else {
                            vec![]
                        },
                        vec!["only explicitly requested evidence claims can become eligible"],
                    ),
                    "card_render" => (
                        card_ok,
                        if card.is_none() {
                            vec!["card_input_missing"]
                        } else if !card_ok {
                            vec!["card_render_failed"]
                        } else {
                            vec![]
                        },
                        vec!["a rendered card may intentionally retain a withheld score"],
                    ),
                    "numeric_card_score" => (
                        card_ok && card_score_attached && evidence_ready,
                        {
                            let mut blockers = Vec::new();
                            if !card_ok {
                                blockers.push("card_render_failed");
                            }
                            if card_ok && !card_score_attached {
                                blockers.push("numeric_score_not_attached");
                            }
                            if evidence_audit.is_none() {
                                blockers.push("evidence_audit_missing");
                            } else if !evidence_ready {
                                blockers.push("evidence_claims_not_ready");
                            }
                            blockers
                        },
                        vec![
                            "numeric score release additionally requires an evidence-conditioned claim audit",
                        ],
                    ),
                    "leaderboard" => (
                        leaderboard_ok && ranked_count > 0,
                        {
                            let mut blockers = Vec::new();
                            if leaderboard.is_none() {
                                blockers.push("leaderboard_input_missing");
                            } else if !leaderboard_ok {
                                blockers.push("leaderboard_render_failed");
                            } else if ranked_count == 0 {
                                blockers.push("no_ranked_entries");
                            }
                            blockers
                        },
                        vec!["ranked and unranked entries remain separately visible"],
                    ),
                    "ranked_leaderboard" => (
                        leaderboard_ok && ranked_count > 0 && unranked_count == 0,
                        {
                            let mut blockers = Vec::new();
                            if leaderboard.is_none() {
                                blockers.push("leaderboard_input_missing");
                            } else if !leaderboard_ok {
                                blockers.push("leaderboard_render_failed");
                            } else if ranked_count == 0 {
                                blockers.push("no_ranked_entries");
                            }
                            if leaderboard_ok && unranked_count > 0 {
                                blockers.push("unranked_entries_present");
                            }
                            blockers
                        },
                        vec!["a fully ranked board requires zero unranked entries"],
                    ),
                    other => {
                        return Err(format!(
                            "unknown release target {other:?}; choose atlas_profile, atlas_aggregation, evidence_claims, card_render, numeric_card_score, leaderboard, or ranked_leaderboard"
                        ));
                    }
                };
                target_rows.push(json!({
                    "target": target,
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
                "reason": "release_request is required to make a publication readiness claim",
                "no_implicit_release": true,
            })
        };

        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/bioatlas-publication-audit/0.1",
            "workflow": "bioatlas_publication_audit",
            "atlas": atlas,
            "evidence_audit": evidence_audit,
            "card": card,
            "leaderboard": leaderboard,
            "release_request": release_request,
            "cross_layer": {
                "numeric_score_requires_evidence_audit": true,
                "numeric_score_evidence_ready": evidence_ready,
                "atlas_aggregation_ready": atlas_aggregation_ready,
                "leaderboard_ranked_count": ranked_count,
                "leaderboard_unranked_count": unranked_count,
                "unranked_leaderboard_entries_remain_visible": true,
                "withheld_scores_are_not_zeroes": true,
            },
            "guarantees": [
                "atlas coverage, evidence-conditioned claims, moderation/card rendering, and leaderboard ranking remain distinct gates",
                "a publication readiness claim is emitted only for explicit requested targets",
                "numeric card scores require both the card's disclosure/publication gate and an evidence-conditioned claim audit",
                "partial or unranked public surfaces remain visible and cannot be silently promoted to a complete release",
            ],
            "limitations": [
                "all atlas, evidence, moderation, disclosure, score, and leaderboard inputs are caller-supplied serialized contracts",
                "this workflow does not publish a web page, authenticate identities, execute assays, or discover leakage",
                "a green render or release target is contract eligibility, not scientific truth, clinical authority, or network deployment",
            ],
        }))
    }

    pub(super) fn pack_health_assess(&self, arguments: &Value) -> Result<Value, String> {
        let raw_pack = arguments
            .get("pack")
            .cloned()
            .ok_or("pack is required and must be a serialized PackIr")?;
        let raw_observations = arguments
            .get("observations")
            .cloned()
            .ok_or("observations is required and must be serialized pack observations")?;
        let raw_policy = arguments.get("policy").cloned();
        let encoded = serde_json::to_vec(&json!({
            "pack": raw_pack.clone(),
            "observations": raw_observations.clone(),
            "policy": raw_policy.clone(),
        }))
        .map_err(|error| format!("cannot measure pack-health envelope: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("pack-health input exceeds the 20000000-byte safety bound".into());
        }

        let pack: PackIr = serde_json::from_value(raw_pack)
            .map_err(|error| format!("invalid pack IR: {error}"))?;
        if let Err(error) = pack.validate() {
            return Ok(json!({
                "ok": false,
                "stage": "pack_validation",
                "refusal": error.to_string(),
                "fail_closed": true,
                "score": Value::Null,
                "guarantees": [
                    "a pack that is not internally valid cannot be health-assessed",
                    "no score is emitted for a malformed or unvalidated pack revision",
                ],
            }));
        }
        let observations: PackObservations = serde_json::from_value(raw_observations)
            .map_err(|error| format!("invalid pack observations: {error}"))?;
        let policy = match raw_policy {
            Some(raw) => serde_json::from_value::<HealthPolicy>(raw)
                .map_err(|error| format!("invalid pack health policy: {error}"))?,
            None => HealthPolicy::default(),
        };

        let assessment = match assess_pack_health(&pack, &observations, &policy) {
            Ok(assessment) => assessment,
            Err(error) => {
                return Ok(json!({
                    "ok": false,
                    "stage": "pack_health_assessment",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "score": Value::Null,
                    "guarantees": [
                        "digest, calibration, oracle, contamination, degeneracy, and materialization checks fail closed",
                        "assessment errors are not converted to a zero score",
                    ],
                }));
            }
        };
        let score_gate = match assessment.reportable_score(&pack) {
            Ok(score) => json!({
                "reportable": true,
                "score": score,
            }),
            Err(error) => json!({
                "reportable": false,
                "refusal": error.to_string(),
                "fail_closed": true,
            }),
        };
        Ok(json!({
            "ok": true,
            "pack": assessment.pack,
            "pack_digest": assessment.pack_digest,
            "verdict": assessment.health.verdict(),
            "finding_count": assessment.health.findings.len(),
            "blocking_findings": assessment.health.blocking().len(),
            "advisory_findings": assessment.health.advisories().len(),
            "health": assessment.health,
            "calibration": assessment.calibration,
            "score_gate": score_gate,
            "guarantees": [
                "health is bound to the immutable pack digest that was assessed",
                "blocking findings prevent any numeric score from being returned",
                "advisories remain attached to a reportable score rather than being dropped",
                "declarations, observed outcomes, oracle posture, and reportability remain separate",
            ],
        }))
    }

    pub(super) fn pack_coverage_audit(&self, arguments: &Value) -> Result<Value, String> {
        let section = arguments
            .get("section")
            .and_then(Value::as_str)
            .unwrap_or("all");
        if !matches!(section, "all" | "15" | "29") {
            return Err("section must be one of all, 15, or 29".into());
        }
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if max_items == 0 || max_items > 1_000 {
            return Err("max_items must be between 1 and 1000".into());
        }
        let requested_ids = arguments.get("pack_ids").and_then(Value::as_array);
        if let Some(ids) = requested_ids {
            if ids.is_empty() || ids.len() > 100 {
                return Err("pack_ids must contain between 1 and 100 ids when supplied".into());
            }
        }
        let mut selected = Vec::new();
        let mut selected_ids = BTreeSet::new();
        let mut unknown = Vec::new();
        if let Some(ids) = requested_ids {
            for id in ids {
                let id = id.as_str().ok_or("pack_ids must be an array of strings")?;
                if !selected_ids.insert(id.to_string()) {
                    return Err(format!("pack_ids contains duplicate id {id}"));
                }
                match all_packs().iter().find(|pack| pack.id == id) {
                    Some(pack)
                        if section == "all"
                            || (section == "15" && pack.blueprint_module.starts_with("15."))
                            || (section == "29" && pack.blueprint_module.starts_with("29.")) =>
                    {
                        selected.push(pack);
                    }
                    Some(_) => {}
                    None => unknown.push(id.to_string()),
                }
            }
        } else {
            selected = all_packs()
                .iter()
                .filter(|pack| {
                    section == "all"
                        || (section == "15" && pack.blueprint_module.starts_with("15."))
                        || (section == "29" && pack.blueprint_module.starts_with("29."))
                })
                .collect();
            selected_ids.extend(selected.iter().map(|pack| pack.id.to_string()));
        }
        if !unknown.is_empty() {
            return Ok(json!({
                "ok": false,
                "schema": "bioprism-mcp/pack-coverage-audit/0.1",
                "stage": "pack_selection",
                "unknown_pack_ids": unknown,
                "refusal": "coverage cannot be computed for unknown pack identifiers",
                "fail_closed": true,
                "guarantees": [
                    "an unknown pack is not silently dropped from a coverage denominator",
                    "coverage rows describe only the explicitly selected portfolio",
                ],
            }));
        }
        if selected.is_empty() {
            return Ok(json!({
                "ok": false,
                "schema": "bioprism-mcp/pack-coverage-audit/0.1",
                "stage": "pack_selection",
                "refusal": "the selected section and pack_ids intersection is empty",
                "fail_closed": true,
                "guarantees": ["an empty portfolio is not rendered as zero coverage"],
            }));
        }
        let report = pack_coverage(&selected);
        let matrix = pack_coverage_matrix(&selected);
        let max_items = max_items as usize;
        let covered = report
            .rows
            .iter()
            .filter(|row| !row.packs.is_empty())
            .count();
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/pack-coverage-audit/0.1",
            "section": section,
            "selected_pack_count": selected.len(),
            "selected_pack_ids": selected_ids,
            "summary": {
                "families": report.rows.len(),
                "covered": covered,
                "uncovered": report.uncovered.len(),
                "singly_covered": report.singly_covered.len(),
                "weakly_covered": report.weakly_covered.len(),
                "coverage_fraction": covered as f64 / report.rows.len().max(1) as f64,
                "gap_summary": report.gap_summary(),
            },
            "rows": report.rows.iter().take(max_items).collect::<Vec<_>>(),
            "rows_omitted": report.rows.len().saturating_sub(max_items),
            "uncovered": report.uncovered.iter().take(max_items).collect::<Vec<_>>(),
            "uncovered_omitted": report.uncovered.len().saturating_sub(max_items),
            "singly_covered": report.singly_covered.iter().take(max_items).collect::<Vec<_>>(),
            "singly_covered_omitted": report.singly_covered.len().saturating_sub(max_items),
            "weakly_covered": report.weakly_covered.iter().take(max_items).collect::<Vec<_>>(),
            "weakly_covered_omitted": report.weakly_covered.len().saturating_sub(max_items),
            "matrix": matrix.iter().take(max_items).collect::<Vec<_>>(),
            "matrix_omitted": matrix.len().saturating_sub(max_items),
            "guarantees": [
                "uncovered, singly covered, weakly covered, and execution-grounded families remain separate",
                "coverage is computed by the packs kernel over the selected portfolio, not by counting catalogue rows",
                "omission counts remain visible for every bounded projection",
            ],
            "limitations": [
                "this is declaration-level portfolio coverage; it does not execute instances or apply health findings to remove packs",
                "maintainer, refresh, and registry state are outside PackDefinition and are not fabricated here",
            ],
        }))
    }

    pub(super) fn pack_release_audit(&self, arguments: &Value) -> Result<Value, String> {
        let section = arguments
            .get("section")
            .and_then(Value::as_str)
            .unwrap_or("all");
        if !matches!(section, "all" | "15" | "29") {
            return Err("section must be one of all, 15, or 29".into());
        }
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if max_items == 0 || max_items > 1_000 {
            return Err("max_items must be between 1 and 1000".into());
        }
        let requested_ids = arguments.get("pack_ids").and_then(Value::as_array);
        if let Some(ids) = requested_ids {
            if ids.is_empty() || ids.len() > 100 {
                return Err("pack_ids must contain between 1 and 100 ids when supplied".into());
            }
        }
        let section_matches = |pack: &bioprism_packs::PackDefinition| {
            section == "all"
                || (section == "15" && pack.blueprint_module.starts_with("15."))
                || (section == "29" && pack.blueprint_module.starts_with("29."))
        };
        let mut selected = Vec::new();
        let mut selected_ids = BTreeSet::new();
        let mut unknown = Vec::new();
        let mut out_of_section = Vec::new();
        if let Some(ids) = requested_ids {
            for id in ids {
                let id = id.as_str().ok_or("pack_ids must be an array of strings")?;
                if !selected_ids.insert(id.to_string()) {
                    return Err(format!("pack_ids contains duplicate id {id}"));
                }
                match all_packs().iter().find(|pack| pack.id == id) {
                    Some(pack) if section_matches(pack) => selected.push(pack),
                    Some(_) => out_of_section.push(id.to_string()),
                    None => unknown.push(id.to_string()),
                }
            }
        } else {
            selected = all_packs()
                .iter()
                .filter(|pack| section_matches(pack))
                .collect();
            selected_ids.extend(selected.iter().map(|pack| pack.id.to_string()));
        }
        if !unknown.is_empty() || !out_of_section.is_empty() {
            return Ok(json!({
                "ok": false,
                "schema": "bioprism-mcp/pack-release-audit/0.1",
                "stage": "pack_selection",
                "unknown_pack_ids": unknown,
                "out_of_section_pack_ids": out_of_section,
                "refusal": "release order cannot be computed for an unknown or section-incompatible pack selection",
                "fail_closed": true,
                "guarantees": [
                    "an invalid pack is not silently dropped from the release denominator",
                    "section-incompatible identifiers are reported rather than reassigned",
                ],
            }));
        }
        if selected.is_empty() {
            return Ok(json!({
                "ok": false,
                "schema": "bioprism-mcp/pack-release-audit/0.1",
                "stage": "pack_selection",
                "refusal": "the selected section and pack_ids intersection is empty",
                "fail_closed": true,
                "guarantees": ["an empty portfolio is not rendered as a complete release plan"],
            }));
        }
        let selected_ids: BTreeSet<String> =
            selected.iter().map(|pack| pack.id.to_string()).collect();
        let global_release = pack_release_order();
        let global_positions: BTreeMap<&str, usize> = global_release
            .iter()
            .enumerate()
            .map(|(index, pack)| (pack.id, index + 1))
            .collect();
        let selected_release = global_release
            .iter()
            .filter(|pack| selected_ids.contains(pack.id))
            .collect::<Vec<_>>();
        let global_unsequenced = pack_unsequenced();
        let selected_unsequenced = global_unsequenced
            .iter()
            .filter(|pack| selected_ids.contains(pack.id))
            .collect::<Vec<_>>();
        let max_items = max_items as usize;
        let release_rows = selected_release
            .iter()
            .enumerate()
            .map(|(index, pack)| {
                json!({
                    "selected_position": index + 1,
                    "portfolio_position": global_positions.get(pack.id).copied(),
                    "id": pack.id,
                    "title": pack.title,
                    "blueprint_module": pack.blueprint_module,
                    "axis": pack.axis,
                    "release_wave": pack.release_wave,
                    "strongest_oracle": pack.strongest_oracle(),
                    "has_execution_grounded_oracle": pack.has_grounded_oracle(),
                })
            })
            .collect::<Vec<_>>();
        let unsequenced_rows = selected_unsequenced
            .iter()
            .map(|pack| {
                json!({
                    "id": pack.id,
                    "title": pack.title,
                    "blueprint_module": pack.blueprint_module,
                    "axis": pack.axis,
                    "release_wave": pack.release_wave,
                    "strongest_oracle": pack.strongest_oracle(),
                    "has_execution_grounded_oracle": pack.has_grounded_oracle(),
                })
            })
            .collect::<Vec<_>>();
        let mut wave_counts = BTreeMap::new();
        let mut axis_counts = BTreeMap::new();
        for pack in &selected {
            *axis_counts
                .entry(format!("{:?}", pack.axis).to_ascii_lowercase())
                .or_insert(0usize) += 1;
            if let bioprism_packs::ReleaseWave::Wave(wave) = pack.release_wave {
                *wave_counts.entry(wave.to_string()).or_insert(0usize) += 1;
            }
        }
        let sequenced_count = selected_release.len();
        let unsequenced_count = selected_unsequenced.len();
        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/pack-release-audit/0.1",
            "section": section,
            "selected_pack_count": selected.len(),
            "selected_pack_ids": selected_ids,
            "sequenced_count": sequenced_count,
            "unsequenced_count": unsequenced_count,
            "release_coverage_fraction": sequenced_count as f64 / selected.len() as f64,
            "wave_counts": wave_counts,
            "axis_counts": axis_counts,
            "release_order": release_rows.iter().take(max_items).collect::<Vec<_>>(),
            "release_order_omitted": release_rows.len().saturating_sub(max_items),
            "unsequenced": unsequenced_rows.iter().take(max_items).collect::<Vec<_>>(),
            "unsequenced_omitted": unsequenced_rows.len().saturating_sub(max_items),
            "guarantees": [
                "release order is the stable portfolio kernel order with wave/module tie-breaking",
                "unsequenced packs remain explicit and are not assigned an invented wave",
                "selected positions, global positions, wave counts, and omission counts remain visible",
            ],
            "limitations": [
                "this is a blueprint release-order projection, not an approval, registry admission, or deployment action",
                "it does not infer dependencies, staffing, calibration, ownership, or readiness for an unsequenced pack",
                "release-wave membership is declaration-level and does not measure pack quality or execution performance",
            ],
        }))
    }

    pub(super) fn foundation_contract_check(&self, arguments: &Value) -> Result<Value, String> {
        let raw_contract = arguments
            .get("contract")
            .cloned()
            .ok_or("contract is required and must be a serialized ContractDraft")?;
        let encoded = serde_json::to_vec(&json!({
            "contract": raw_contract.clone(),
            "parent": arguments.get("parent"),
            "envelope": arguments.get("envelope"),
            "world": arguments.get("world"),
            "transition": arguments.get("transition"),
        }))
        .map_err(|error| format!("cannot measure foundation envelope: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("foundation input exceeds the 20000000-byte safety bound".into());
        }

        let draft: ContractDraft = serde_json::from_value(raw_contract)
            .map_err(|error| format!("invalid contract draft: {error}"))?;
        let contract = match FalsifiableContract::admit(draft) {
            Ok(contract) => contract,
            Err(error) => {
                return Ok(json!({
                    "ok": true,
                    "verdict": "refused",
                    "contract": { "ok": false, "refusal": error.to_string(), "fail_closed": true },
                    "guarantees": [
                        "a contract without falsifiers, actions, claim schema, reference standard, or terminal states never becomes a checked contract",
                        "partial contract fields are not presented as an evaluable result",
                    ],
                }));
            }
        };

        let parent_check = if let Some(raw_parent) = arguments.get("parent") {
            let parent_draft: ContractDraft = serde_json::from_value(raw_parent.clone())
                .map_err(|error| format!("invalid parent contract draft: {error}"))?;
            match FalsifiableContract::admit(parent_draft) {
                Ok(parent) => match contract.refines(&parent) {
                    Ok(()) => json!({ "ok": true, "relation": "refines" }),
                    Err(error) => json!({
                        "ok": false,
                        "relation": "refused",
                        "refusal": error.to_string(),
                        "fail_closed": true,
                    }),
                },
                Err(error) => json!({
                    "ok": false,
                    "relation": "refused",
                    "refusal": format!("parent is not admissible: {error}"),
                    "fail_closed": true,
                }),
            }
        } else {
            Value::Null
        };

        let envelope_check = if let Some(raw_envelope) = arguments.get("envelope") {
            let envelope: ApplicabilityEnvelope = serde_json::from_value(raw_envelope.clone())
                .map_err(|error| format!("invalid applicability envelope: {error}"))?;
            let structure = envelope.check();
            let maturity = if arguments
                .get("present_as_established")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                envelope.present_as_established()
            } else {
                Ok(())
            };
            let structure_message = match &structure {
                Ok(()) => "complete".to_string(),
                Err(error) => error.to_string(),
            };
            let maturity_message = match &maturity {
                Ok(()) => "not_requested_or_admissible".to_string(),
                Err(error) => error.to_string(),
            };
            json!({
                "ok": structure.is_ok() && maturity.is_ok(),
                "structure": structure_message,
                "maturity": maturity_message,
                "maturity_rung": envelope.maturity.rung(),
                "fail_closed": structure.is_err() || maturity.is_err(),
            })
        } else {
            Value::Null
        };

        let world_check = if let Some(raw_world) = arguments.get("world") {
            let world: BioWorldDeclaration = serde_json::from_value(raw_world.clone())
                .map_err(|error| format!("invalid BioWorld declaration: {error}"))?;
            let reveal = world.check_reveal_policy();
            let claim_check = if let Some(raw_claim) = arguments.get("claim") {
                let claim: CounterfactualClaim = serde_json::from_value(raw_claim.clone())
                    .map_err(|error| format!("invalid counterfactual claim: {error}"))?;
                Some(world.admits(claim))
            } else {
                None
            };
            let claim_message = claim_check.as_ref().map(|result| match result {
                Ok(()) => "admitted".to_string(),
                Err(error) => error.to_string(),
            });
            let reveal_message = match &reveal {
                Ok(()) => "admissible".to_string(),
                Err(error) => error.to_string(),
            };
            let claim_ok = claim_check.as_ref().is_none_or(Result::is_ok);
            json!({
                "ok": reveal.is_ok() && claim_ok,
                "world_id": world.id,
                "class": world.class,
                "counterfactual_strength": world.class.counterfactual_strength(),
                "reveal_policy": reveal_message,
                "claim": claim_message,
                "fail_closed": reveal.is_err() || !claim_ok,
            })
        } else {
            Value::Null
        };

        let transition_check = if let Some(raw_transition) = arguments.get("transition") {
            let transition: Transition = serde_json::from_value(raw_transition.clone())
                .map_err(|error| format!("invalid world transition: {error}"))?;
            match transition.check() {
                Ok(()) => json!({ "ok": true, "verdict": "plane_consistent" }),
                Err(error) => json!({
                    "ok": false,
                    "verdict": "plane_confusion",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                }),
            }
        } else {
            Value::Null
        };
        let relation_ok = parent_check
            .get("ok")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let envelope_ok = envelope_check
            .get("ok")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let world_ok = world_check
            .get("ok")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let transition_ok = transition_check
            .get("ok")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        Ok(json!({
            "ok": true,
            "verdict": if relation_ok && envelope_ok && world_ok && transition_ok { "admitted" } else { "refused" },
            "contract": {
                "ok": true,
                "id": contract.id(),
                "intent": contract.intent(),
                "falsifier_count": contract.falsifiers().count(),
                "action_count": contract.actions().count(),
                "evidence_obligation_count": contract.evidence_obligations().count(),
                "minimum_reviewers": contract.minimum_reviewers(),
                "uncertainty_required": contract.uncertainty_required(),
            },
            "parent_relation": parent_check,
            "envelope": envelope_check,
            "world": world_check,
            "transition": transition_check,
            "guarantees": [
                "contract admissibility, inheritance, applicability, world-class claim strength, and transition-plane checks remain separate gates",
                "out-of-envelope, weakened refinements, unsupported counterfactuals, missing reveal policy, and plane confusion remain typed refusals",
                "this endpoint validates declarations; it does not execute a world, infer evidence, or manufacture treatment authority",
            ],
        }))
    }

    pub(super) fn atlas_report(&self, arguments: &Value) -> Result<Value, String> {
        let raw = arguments
            .get("atlas")
            .cloned()
            .ok_or("atlas is required and must be a serialized bioprism-atlas Atlas")?;
        let raw_bytes = serde_json::to_vec(&raw).map_err(|error| error.to_string())?;
        if raw_bytes.len() > 10_000_000 {
            return Err("atlas exceeds the 10000000-byte safety bound".into());
        }
        let atlas: Atlas =
            serde_json::from_value(raw).map_err(|error| format!("invalid atlas: {error}"))?;
        let report = CoverageReport::of(&atlas);
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if max_items == 0 || max_items > 1_000 {
            return Err("max_items must be between 1 and 1000".into());
        }
        let max_items = max_items as usize;

        let composite = if let Some(raw_policy) = arguments.get("weighting") {
            let policy: WeightingPolicy = serde_json::from_value(raw_policy.clone())
                .map_err(|error| format!("invalid weighting policy: {error}"))?;
            match atlas_composite(&atlas, &policy) {
                Ok(value) => json!({ "ok": true, "value": value }),
                Err(error) => json!({
                    "ok": false,
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "guarantee": "unmeasured, confounded, or below-tier capabilities never become a numeric composite"
                }),
            }
        } else {
            Value::Null
        };

        Ok(json!({
            "ok": true,
            "schema": "bioprism-mcp/atlas-report/0.1",
            "ontology_version": report.ontology_version.as_str(),
            "summary": {
                "measured": report.measured.len(),
                "holes": report.holes.len(),
                "families": report.family_coverage.len(),
                "inconsistencies": report.inconsistencies.len(),
                "coverage_debt_ratio": report.debt.ratio(),
                "has_holes": report.has_holes(),
                "coverage_supports_aggregation": report.coverage_supports_aggregation(),
            },
            "debt": report.debt,
            "measured": report.measured.iter().take(max_items).collect::<Vec<_>>(),
            "omitted_measured": report.measured.len().saturating_sub(max_items),
            "holes": report.holes.iter().take(max_items).collect::<Vec<_>>(),
            "omitted_holes": report.holes.len().saturating_sub(max_items),
            "family_coverage": report.family_coverage.iter().take(max_items).collect::<Vec<_>>(),
            "omitted_families": report.family_coverage.len().saturating_sub(max_items),
            "depth_histogram": report.depth_histogram.iter().take(max_items).collect::<Vec<_>>(),
            "stage_histogram": report.first_divergence_histogram.iter().take(max_items).collect::<Vec<_>>(),
            "inconsistencies": report.inconsistencies.iter().take(max_items).collect::<Vec<_>>(),
            "omitted_inconsistencies": report.inconsistencies.len().saturating_sub(max_items),
            "composite": composite,
            "guarantees": [
                "unmeasured capabilities remain holes and are never rendered as zero",
                "coverage debt distinguishes measured poor, unmeasured, declared out-of-scope, unclassified, and undiagnosed states",
                "composite eligibility delegates to ontology, measurement, confounding, and claim-tier gates",
            ],
            "limitations": [
                "the atlas indexes caller-supplied evidence; it does not run trials or estimate metrics",
                "confidence intervals, stratification beyond the atlas record, and public visual panels are outside this contract",
            ]
        }))
    }

    pub(super) fn atlas_surface_audit(&self, arguments: &Value) -> Result<Value, String> {
        const SCHEMA: &str = "bioprism-mcp/atlas-surface-audit/0.1";
        const MAX_INPUT_BYTES: usize = 20_000_000;
        const MAX_FAILURES: usize = 8_192;
        const MAX_VISIBILITY: usize = 8_192;
        const MAX_RATE_CAPABILITIES: usize = 4_096;

        let refusal = |stage: &str, detail: String| {
            json!({
                "ok": false,
                "schema": SCHEMA,
                "workflow": "atlas_surface_audit",
                "stage": stage,
                "refusal": detail,
                "fail_closed": true,
                "guarantees": [
                    "coverage debt is derived from a serialized CapabilityGrid rather than caller-supplied counts",
                    "declared-away holes remain distinct from measured holes during discharge",
                    "failure buckets retain record identifiers, withheld publication states, and denominator limitations",
                    "a failure browse never becomes a rate without a CapabilityGrid denominator",
                    "surface soundness is checked against the atlasx declaration and undeclared-question probes",
                ],
                "limitations": [
                    "the route does not execute assays, infer ontology coverage, or dereference failure evidence",
                    "rates are failure-count over effective-size projections, not trial-level incidence unless the grid defines that denominator",
                    "publication visibility is a caller-supplied declaration and is not an access-control enforcement point",
                ],
            })
        };

        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot measure atlas surface input: {error}"))?;
        if encoded.len() > MAX_INPUT_BYTES {
            return Err(format!(
                "atlas surface input exceeds the {MAX_INPUT_BYTES}-byte safety bound"
            ));
        }
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if !(1..=1_000).contains(&max_items) {
            return Err("max_items must be between 1 and 1000".into());
        }
        let max_items = max_items as usize;
        let policy = |name: &str| -> Result<bool, String> {
            match arguments.get(name) {
                None => Ok(false),
                Some(value) => value
                    .as_bool()
                    .ok_or_else(|| format!("{name} must be a boolean")),
            }
        };
        let require_no_holes = policy("require_no_holes")?;
        let require_no_blocking_debt = policy("require_no_blocking_debt")?;
        let require_no_withheld = policy("require_no_withheld")?;
        let require_sound_surfaces = policy("require_sound_surfaces")?;

        let raw_grid = arguments
            .get("grid")
            .cloned()
            .ok_or("grid is required and must be a serialized CapabilityGrid")?;
        let grid: CapabilityGrid = match serde_json::from_value(raw_grid) {
            Ok(grid) => grid,
            Err(error) => {
                return Ok(refusal(
                    "grid_deserialization",
                    format!("grid is not a valid CapabilityGrid: {error}"),
                ));
            }
        };
        let debt = AtlasxDebtStatement::of(&grid);
        let debt_audit = atlasx_audit(&debt);
        let debt_audit_value =
            serde_json::to_value(&debt_audit).map_err(|error| error.to_string())?;

        let discharge = match arguments.get("later_grid") {
            None | Some(Value::Null) => Value::Null,
            Some(raw_later) => {
                let later: CapabilityGrid = match serde_json::from_value(raw_later.clone()) {
                    Ok(later) => later,
                    Err(error) => {
                        return Ok(refusal(
                            "later_grid_deserialization",
                            format!("later_grid is not a valid CapabilityGrid: {error}"),
                        ));
                    }
                };
                let later_debt = AtlasxDebtStatement::of(&later);
                let discharge = match debt.discharged_by(&later_debt) {
                    Ok(discharge) => discharge,
                    Err(error) => {
                        return Ok(refusal(
                            "debt_discharge",
                            format!("cannot compare grid coverage: {error}"),
                        ));
                    }
                };
                let bounded = |values: &[String]| {
                    json!({
                        "rows": values.iter().take(max_items).collect::<Vec<_>>(),
                        "total": values.len(),
                        "omitted": values.len().saturating_sub(max_items),
                    })
                };
                json!({
                    "subject": discharge.subject,
                    "measured": bounded(&discharge.measured),
                    "declared_away": bounded(&discharge.declared_away),
                    "persisting": bounded(&discharge.persisting),
                    "newly_unmeasured": bounded(&discharge.newly_unmeasured),
                    "any_evidence": discharge.any_evidence(),
                })
            }
        };
        if require_no_holes && debt.unmeasured() > 0 {
            return Ok(refusal(
                "coverage_policy",
                format!(
                    "require_no_holes was true but {} capability hole(s) remain",
                    debt.unmeasured()
                ),
            ));
        }
        if require_no_blocking_debt && !debt.blocking().is_empty() {
            return Ok(refusal(
                "blocking_debt_policy",
                format!(
                    "require_no_blocking_debt was true but {} hole(s) still block claims",
                    debt.blocking().len()
                ),
            ));
        }

        let raw_failures = match arguments.get("failures") {
            None => Vec::new(),
            Some(value) => value
                .as_array()
                .cloned()
                .ok_or("failures must be an array of serialized FailureRecord values")?,
        };
        if raw_failures.len() > MAX_FAILURES {
            return Err("failures are bounded at 8192 records".into());
        }
        let mut failures = Vec::with_capacity(raw_failures.len());
        for (index, raw) in raw_failures.into_iter().enumerate() {
            match serde_json::from_value::<FailureRecord>(raw) {
                Ok(record) => failures.push(record),
                Err(error) => {
                    return Ok(refusal(
                        "failure_deserialization",
                        format!("failures[{index}] is not a valid FailureRecord: {error}"),
                    ));
                }
            }
        }
        let facet = match arguments.get("facet") {
            None => AtlasxFacet::Mechanism,
            Some(raw) => match serde_json::from_value::<AtlasxFacet>(raw.clone()) {
                Ok(facet) => facet,
                Err(error) => {
                    return Ok(refusal(
                        "facet_deserialization",
                        format!("facet is not a valid atlasx Facet: {error}"),
                    ));
                }
            },
        };
        let raw_visibility = match arguments.get("visibility") {
            None => Vec::new(),
            Some(value) => value
                .as_array()
                .cloned()
                .ok_or("visibility must be an array of {failure_id,state} values")?,
        };
        if raw_visibility.len() > MAX_VISIBILITY {
            return Err("visibility is bounded at 8192 declarations".into());
        }
        let mut visibility = Vec::with_capacity(raw_visibility.len());
        for (index, raw) in raw_visibility.into_iter().enumerate() {
            match serde_json::from_value::<AtlasxVisibility>(raw) {
                Ok(declaration) => visibility.push(declaration),
                Err(error) => {
                    return Ok(refusal(
                        "visibility_deserialization",
                        format!("visibility[{index}] is not valid: {error}"),
                    ));
                }
            }
        }
        let browse_subject = arguments
            .get("failure_subject")
            .and_then(Value::as_str)
            .unwrap_or(grid.label.as_str());
        if browse_subject.trim().is_empty() || browse_subject.len() > 4_096 {
            return Err("failure_subject must be a non-empty string of at most 4096 bytes".into());
        }
        let browse =
            match atlasx_browse_with_visibility(browse_subject, &failures, facet, &visibility) {
                Ok(browse) => browse,
                Err(error) => {
                    return Ok(refusal(
                        "failure_browse",
                        format!("cannot construct failure browse: {error}"),
                    ));
                }
            };
        if require_no_withheld && browse.withheld() > 0 {
            return Ok(refusal(
                "visibility_policy",
                format!(
                    "require_no_withheld was true but {} failure record(s) are withheld",
                    browse.withheld()
                ),
            ));
        }
        let browse_audit = atlasx_audit(&browse);
        if require_sound_surfaces && (!debt_audit.sound() || !browse_audit.sound()) {
            return Ok(refusal(
                "surface_soundness",
                "require_sound_surfaces was true but an atlasx surface answered an undeclared question"
                    .to_string(),
            ));
        }

        let raw_rate_capabilities = match arguments.get("rate_capabilities") {
            None => Vec::new(),
            Some(value) => value
                .as_array()
                .cloned()
                .ok_or("rate_capabilities must be an array of capability identifiers")?,
        };
        if raw_rate_capabilities.len() > MAX_RATE_CAPABILITIES {
            return Err("rate_capabilities are bounded at 4096 identifiers".into());
        }
        let mut rate_rows = Vec::with_capacity(raw_rate_capabilities.len());
        for (index, raw) in raw_rate_capabilities.into_iter().enumerate() {
            let name = raw.as_str().ok_or_else(|| {
                format!("rate_capabilities[{index}] must be a capability identifier string")
            })?;
            let capability: CapabilityId = match serde_json::from_value(json!(name)) {
                Ok(capability) => capability,
                Err(error) => {
                    return Ok(refusal(
                        "rate_capability_deserialization",
                        format!("rate_capabilities[{index}] is not valid: {error}"),
                    ));
                }
            };
            let answer = browse.rate_against(&grid, &capability);
            rate_rows.push(json!({
                "capability": name,
                "answer": &answer,
                "answered": answer.is_answered(),
            }));
        }

        let debt_holes = debt
            .holes()
            .iter()
            .take(max_items)
            .map(|hole| {
                json!({
                    "capability": &hole.capability,
                    "reason": &hole.reason,
                    "blocks_claim": hole.blocks_claim(),
                })
            })
            .collect::<Vec<_>>();
        let blocking_capabilities = debt
            .blocking()
            .into_iter()
            .map(|hole| hole.capability.as_str().to_string())
            .collect::<Vec<_>>();
        let browse_buckets = browse
            .buckets()
            .iter()
            .take(max_items)
            .map(|bucket| {
                json!({
                    "key": &bucket.key,
                    "label": bucket.key.label(),
                    "members": bucket.members.iter().take(max_items).collect::<Vec<_>>(),
                    "member_count": bucket.members.len(),
                    "omitted_members": bucket.members.len().saturating_sub(max_items),
                    "cell": bucket.cell(),
                })
            })
            .collect::<Vec<_>>();
        let coverage_profile = serde_json::to_value(debt.profile_coverage())
            .map_err(|error| format!("cannot serialize coverage profile: {error}"))?;
        let debt_audit_summary = json!({
            "sound": debt_audit.sound(),
            "declared_but_refused": debt_audit.declared_but_refused.len(),
            "undeclared_but_answered": debt_audit.undeclared_but_answered.len(),
            "non_numeric_cells": debt_audit.non_numeric_cells.len(),
        });
        let browse_audit_summary = json!({
            "sound": browse_audit.sound(),
            "declared_but_refused": browse_audit.declared_but_refused.len(),
            "undeclared_but_answered": browse_audit.undeclared_but_answered.len(),
            "non_numeric_cells": browse_audit.non_numeric_cells.len(),
        });
        let rate_total = rate_rows.len();
        let browse_bucket_total = browse.buckets().len();
        let browse_audit_value =
            serde_json::to_value(&browse_audit).map_err(|error| error.to_string())?;
        Ok(json!({
            "ok": true,
            "schema": SCHEMA,
            "workflow": "atlas_surface_audit",
            "coverage": {
                "subject": debt.subject(),
                "total_capabilities": debt.total_capabilities(),
                "measured": debt.measured(),
                "unmeasured": debt.unmeasured(),
                "blocking": blocking_capabilities.len(),
                "closed_by_declaration": debt.closed_by_declaration(),
                "vacuous": debt.is_vacuous(),
                "holes": debt_holes,
                "omitted_holes": debt.unmeasured().saturating_sub(max_items),
                "blocking_capabilities": blocking_capabilities.iter().take(max_items).collect::<Vec<_>>(),
                "omitted_blocking_capabilities": blocking_capabilities.len().saturating_sub(max_items),
                "profile_coverage": coverage_profile,
            },
            "debt_discharge": discharge,
            "failure_browse": {
                "subject": browse.subject(),
                "facet": browse.facet(),
                "taxonomy_version": browse.taxonomy_version(),
                "records_browsed": browse.records_browsed(),
                "visible": browse.visible(),
                "withheld": browse.withheld(),
                "contested": browse.contested(),
                "undiagnosed": browse.undiagnosed(),
                "evaluator_induced": browse.evaluator_induced(),
                "distinct_families": browse.distinct_families(),
                "shares_sum_to_one": browse.shares_sum_to_one(),
                "buckets": browse_buckets,
                "omitted_buckets": browse_bucket_total.saturating_sub(max_items),
            },
            "rate_checks": {
                "rows": rate_rows,
                "total": rate_total,
            },
            "surface_audits": {
                "debt": debt_audit_value,
                "browse": browse_audit_value,
                "summary": {
                    "debt": debt_audit_summary,
                    "browse": browse_audit_summary,
                },
                "sound": debt_audit.sound() && browse_audit.sound(),
            },
            "policies": {
                "require_no_holes": require_no_holes,
                "require_no_blocking_debt": require_no_blocking_debt,
                "require_no_withheld": require_no_withheld,
                "require_sound_surfaces": require_sound_surfaces,
            },
            "guarantees": [
                "coverage percentage carries its grid denominator and refuses the vacuous zero-over-zero case",
                "every returned hole carries its atlas UnmeasuredReason and claim-blocking interpretation",
                "discharge separates measured evidence from declared-away scope changes and newly unmeasured capabilities",
                "withheld failures remain state-bearing buckets and are not emitted as diagnosis counts",
                "browsed failure records remain identifiers that can be reproduced locally rather than anonymous histogram mass",
                "rate checks use the same browse's charged failure counts and the supplied grid's effective-size denominator",
                "surface audits retain declared refusals and non-numeric cells instead of coercing them to zero",
            ],
            "limitations": [
                "the route does not build a second ontology or recalculate atlas measurements",
                "failure taxonomy version is checked within the browse but is not automatically matched to the grid conditions",
                "a rate is a projection over the grid cell's effective size; it is not a causal, clinical, or prevalence estimate",
                "visibility declarations describe the intended publication surface and do not remove source records from memory",
            ],
        }))
    }

    pub(super) fn bundle_verify(&self, arguments: &Value) -> Result<Value, String> {
        let inline = arguments.get("bundle").cloned();
        let publicly_attested = arguments.get("publicly_attested_bundle").cloned();
        let document = arguments.get("document").and_then(Value::as_str);
        let verification_key = arguments.get("verification_key").cloned();
        let trust_registry = arguments.get("trust_registry").cloned();
        let trust_policy = arguments.get("trust_policy").cloned();
        if [
            inline.is_some(),
            publicly_attested.is_some(),
            document.is_some(),
        ]
        .into_iter()
        .filter(|present| *present)
        .count()
            > 1
        {
            return Err(
                "provide either bundle or document, or publicly_attested_bundle as the signed-bundle alternative".into(),
            );
        }
        if verification_key.is_some() && trust_registry.is_some() {
            return Err(
                "verification_key and trust_registry are mutually exclusive verification modes"
                    .into(),
            );
        }
        if trust_registry.is_some() != trust_policy.is_some() {
            return Err("trust_registry and trust_policy must be supplied together".into());
        }
        if publicly_attested.is_some() && verification_key.is_none() && trust_registry.is_none() {
            return Err(
                "publicly_attested_bundle requires verification_key or trust_registry".into(),
            );
        }
        if trust_registry.is_some() && inline.is_some() {
            return Err(
                "trust_registry requires publicly attested bundle input, not a legacy HMAC bundle"
                    .into(),
            );
        }
        let raw = match (inline, publicly_attested, document) {
            (Some(bundle), None, None) => bundle,
            (None, Some(bundle), None) => bundle,
            (None, None, Some(relative)) => {
                let path = self.resolve(relative)?;
                if path.is_dir() {
                    return Err(
                        "bundle_verify requires a JSON bundle document, not a directory".into(),
                    );
                }
                let metadata = std::fs::metadata(&path)
                    .map_err(|error| format!("cannot inspect bundle document: {error}"))?;
                if metadata.len() > 20_000_000 {
                    return Err("bundle document exceeds the 20000000-byte safety bound".into());
                }
                self.read_json(&path)?
            }
            (None, None, None) => return Err("bundle_verify requires bundle or document".into()),
            _ => unreachable!("bundle inputs were checked as exclusive"),
        };
        if verification_key.is_some() || trust_registry.is_some() {
            let signed: PubliclyAttestedBundle = serde_json::from_value(raw)
                .map_err(|error| format!("invalid publicly attested result bundle: {error}"))?;
            if signed.bundle.manifest.entries.len() > 10_000
                || signed.bundle.contents.len() > 10_000
            {
                return Err(
                    "bundle may contain at most 10000 manifest entries and contents".into(),
                );
            }
            if let Some(raw_registry) = trust_registry {
                let registry: KeyRegistry = serde_json::from_value(raw_registry)
                    .map_err(|error| format!("invalid trust registry: {error}"))?;
                let policy: TrustPolicy =
                    serde_json::from_value(trust_policy.ok_or_else(|| {
                        "trust_policy is required with trust_registry".to_string()
                    })?)
                    .map_err(|error| format!("invalid trust policy: {error}"))?;
                return match signed.verify_with_registry(&registry, &policy) {
                    Ok((verified, authenticated, trust_report)) => Ok(json!({
                        "ok": true,
                        "verification_mode": "ed25519_registry_policy",
                        "schema_version": signed.bundle.manifest.schema_version,
                        "bundle_id": signed.bundle.manifest.bundle_id,
                        "manifest_digest": verified.manifest_digest(),
                        "entry_checks": verified.entry_checks(),
                        "not_recomputed": verified.not_recomputed(),
                        "certificate": verified.certificate(),
                        "supply_chain_posture": verified.supply_chain_posture(),
                        "authentication": authenticated,
                        "trust_report": trust_report,
                        "honest_label": authenticated.honest_label(),
                        "guarantees": [
                            "every carried inline digest is recomputed before registry policy accepts the Ed25519 signature",
                            "the registry snapshot authorizes purpose, role, producer binding, validity, delegation, revocation, and rotation",
                            "registry policy is evaluated at the explicit signed_at or as_of instant without reading the verifier clock",
                        ],
                        "limitations": [
                            "the registry is caller-supplied and is not an external identity, transparency, or timestamp authority",
                            "reference locators are not dereferenced; this is a local value check, not closure fetching",
                            "verification establishes cryptographic origin and local authorization, not scientific validity, provenance truth, or deployment security",
                        ],
                    })),
                    Err(error) => Ok(json!({
                        "ok": false,
                        "verification_mode": "ed25519_registry_policy",
                        "stage": "bundle_verification",
                        "refusal": error.to_string(),
                        "fail_closed": true,
                        "guarantee": "a registry, lifecycle, role, producer, digest, signature, purpose, identity, or key-validity mismatch never becomes a successful release result",
                    })),
                };
            }
            let raw_key = verification_key.ok_or_else(|| {
                "verification_key is required for direct public-key verification".to_string()
            })?;
            let key: VerificationKey = serde_json::from_value(raw_key)
                .map_err(|error| format!("invalid Ed25519 verification key: {error}"))?;
            return match signed.verify(&key) {
                Ok((verified, authenticated)) => Ok(json!({
                    "ok": true,
                    "verification_mode": "ed25519_public_key",
                    "schema_version": signed.bundle.manifest.schema_version,
                    "bundle_id": signed.bundle.manifest.bundle_id,
                    "manifest_digest": verified.manifest_digest(),
                    "entry_checks": verified.entry_checks(),
                    "not_recomputed": verified.not_recomputed(),
                    "certificate": verified.certificate(),
                    "supply_chain_posture": verified.supply_chain_posture(),
                    "authentication": authenticated,
                    "honest_label": authenticated.honest_label(),
                    "guarantees": [
                        "every carried inline digest is recomputed before the Ed25519 signature is accepted",
                        "the signature is bound to the manifest digest, purpose, key identity, producer claim, nonce, and signed instant",
                        "bounded key validity is checked against the caller-supplied signed_at value without reading the verifier clock",
                    ],
                    "limitations": [
                        "reference locators are not dereferenced; this is a local value check, not closure fetching",
                        "the public key identity and claimed producer are not authenticated by a registry or certificate chain",
                        "signed_at is caller-supplied and is not timestamp-authority evidence",
                        "verification establishes cryptographic origin of bytes, not scientific validity, provenance truth, or deployment security",
                    ],
                })),
                Err(error) => Ok(json!({
                    "ok": false,
                    "verification_mode": "ed25519_public_key",
                    "stage": "bundle_verification",
                    "refusal": error.to_string(),
                    "fail_closed": true,
                    "guarantee": "a digest, signature, purpose, identity, or key-validity mismatch never becomes a successful release result",
                })),
            };
        }
        let bundle: ResultBundle = serde_json::from_value(raw)
            .map_err(|error| format!("invalid result bundle: {error}"))?;
        if bundle.manifest.entries.len() > 10_000 || bundle.contents.len() > 10_000 {
            return Err("bundle may contain at most 10000 manifest entries and contents".into());
        }
        match bundle.verify() {
            Ok(verified) => Ok(json!({
                "ok": true,
                "schema_version": bundle.manifest.schema_version,
                "bundle_id": bundle.manifest.bundle_id,
                "manifest_digest": verified.manifest_digest(),
                "entry_checks": verified.entry_checks(),
                "not_recomputed": verified.not_recomputed(),
                "certificate": verified.certificate(),
                "supply_chain_posture": verified.supply_chain_posture(),
                "honest_label": verified.honest_label(),
                "guarantees": [
                    "every carried inline digest is recomputed from content rather than trusted from the manifest",
                    "unlisted content, missing inline content, duplicate certificate roles, and invalid embedded certificates are refused",
                    "referenced entries remain visible as not recomputed rather than being reported as verified",
                ],
                "limitations": [
                    "reference locators are not dereferenced; this is a local value check, not closure fetching",
                    "the bundle library authenticates with a symmetric shared secret only, and this tool does not verify an attestation key",
                    "verification establishes internal digest consistency, not scientific validity, provenance truth, or deployment security",
                ],
            })),
            Err(error) => Ok(json!({
                "ok": false,
                "stage": "bundle_verification",
                "refusal": error.to_string(),
                "fail_closed": true,
                "guarantee": "a mismatch or missing closure never becomes a successful release result",
            })),
        }
    }
}
