//! MCP Evaluator, estimand, and grounding handlers.
//!
//! Kept beside the dispatcher so each domain surface can be reviewed independently.

use super::*;

impl Server {
    pub(super) fn bioeval_grounding_audit(&self, arguments: &Value) -> Result<Value, String> {
        const SCHEMA: &str = "bioprism-mcp/bioeval-grounding-audit/0.1";
        const MAX_GRAPH_ROWS: usize = 4096;
        const MAX_ID_BYTES: usize = 256;

        let raw_claims = arguments
            .get("claims")
            .and_then(Value::as_array)
            .ok_or("claims is required and must be an array of {id} objects")?;
        let raw_evidence = arguments
            .get("evidence")
            .and_then(Value::as_array)
            .ok_or("evidence is required and must be an array of evidence objects")?;
        let raw_edges = arguments
            .get("edges")
            .and_then(Value::as_array)
            .ok_or("edges is required and must be an array of support-edge objects")?;
        if raw_claims.len() > MAX_GRAPH_ROWS
            || raw_evidence.len() > MAX_GRAPH_ROWS
            || raw_edges.len() > MAX_GRAPH_ROWS
        {
            return Err("claims, evidence, and edges are each bounded at 4096 rows".into());
        }
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot measure grounding audit input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("grounding audit input exceeds the 20000000-byte safety bound".into());
        }
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if !(1..=1_000).contains(&max_items) {
            return Err("max_items must be between 1 and 1000".into());
        }
        let max_items = max_items as usize;

        let refusal = |stage: &str, detail: String| {
            json!({
                "ok": false,
                "schema": SCHEMA,
                "workflow": "bioeval_grounding_audit",
                "stage": stage,
                "refusal": detail,
                "fail_closed": true,
                "guarantees": [
                    "claims, evidence, and typed edges are validated before any grounding state is reported",
                    "domain-invalid graphs never become partially grounded success projections",
                    "unverified, contradicted, contested, and unsupported claims remain distinct states",
                ],
                "limitations": [
                    "the route does not dereference locators, verify digests, extract claims, or judge assay compatibility",
                    "staleness is evaluated only against an explicit caller-supplied freeze",
                    "lineage completeness is a recorded-chain predicate, not a specimen registry lookup",
                ],
            })
        };

        let mut graph = Grounding::new();
        for (index, raw) in raw_claims.iter().enumerate() {
            let id = match raw.get("id").and_then(Value::as_str) {
                Some(id) if !id.trim().is_empty() && id.len() <= MAX_ID_BYTES => id,
                Some(_) => {
                    return Ok(refusal(
                        "claim_validation",
                        format!("claims[{index}].id must contain 1 to {MAX_ID_BYTES} bytes"),
                    ));
                }
                None => {
                    return Ok(refusal(
                        "claim_validation",
                        format!("claims[{index}] requires an id string"),
                    ));
                }
            };
            if let Err(error) = graph.claim(id) {
                return Ok(refusal("claim_validation", error.to_string()));
            }
        }

        let mut evidence_records: BTreeMap<
            String,
            (FactoryTimestamp, Vec<String>, GroundingLocatorStatus),
        > = BTreeMap::new();
        for (index, raw) in raw_evidence.iter().enumerate() {
            let id = match raw.get("id").and_then(Value::as_str) {
                Some(id) if !id.trim().is_empty() && id.len() <= MAX_ID_BYTES => id,
                Some(_) => {
                    return Ok(refusal(
                        "evidence_validation",
                        format!("evidence[{index}].id must contain 1 to {MAX_ID_BYTES} bytes"),
                    ));
                }
                None => {
                    return Ok(refusal(
                        "evidence_validation",
                        format!("evidence[{index}] requires an id string"),
                    ));
                }
            };
            let last_modified = match raw.get("last_modified").and_then(Value::as_str) {
                Some(value) => match FactoryTimestamp::parse(value) {
                    Ok(timestamp) => timestamp,
                    Err(error) => {
                        return Ok(refusal(
                            "evidence_validation",
                            format!("evidence[{index}].last_modified is invalid: {error}"),
                        ));
                    }
                },
                None => {
                    return Ok(refusal(
                        "evidence_validation",
                        format!("evidence[{index}] requires last_modified as RFC-3339 text"),
                    ));
                }
            };
            let lineage = match raw.get("lineage") {
                None | Some(Value::Null) => Vec::new(),
                Some(value) => {
                    let values = match value.as_array() {
                        Some(values) => values,
                        None => {
                            return Ok(refusal(
                                "evidence_validation",
                                format!("evidence[{index}].lineage must be an array of strings"),
                            ));
                        }
                    };
                    if values.len() > MAX_GRAPH_ROWS {
                        return Ok(refusal(
                            "evidence_validation",
                            format!(
                                "evidence[{index}].lineage is bounded at {MAX_GRAPH_ROWS} entries"
                            ),
                        ));
                    }
                    let mut lineage = Vec::with_capacity(values.len());
                    for (lineage_index, value) in values.iter().enumerate() {
                        let ancestor = match value.as_str() {
                            Some(ancestor)
                                if !ancestor.trim().is_empty()
                                    && ancestor.len() <= MAX_ID_BYTES =>
                            {
                                ancestor
                            }
                            _ => {
                                return Ok(refusal(
                                    "evidence_validation",
                                    format!(
                                        "evidence[{index}].lineage[{lineage_index}] must be a non-empty string of at most {MAX_ID_BYTES} bytes"
                                    ),
                                ));
                            }
                        };
                        lineage.push(ancestor.to_string());
                    }
                    lineage
                }
            };
            let locator = match raw.get("locator_status") {
                None | Some(Value::Null) => GroundingLocatorStatus::NotChecked,
                Some(value) => {
                    match serde_json::from_value::<GroundingLocatorStatus>(value.clone()) {
                        Ok(locator) => locator,
                        Err(error) => {
                            return Ok(refusal(
                                "evidence_validation",
                                format!("evidence[{index}].locator_status is invalid: {error}"),
                            ));
                        }
                    }
                }
            };
            let evidence = GroundingEvidence {
                id: id.to_string(),
                lineage: lineage.clone(),
                last_modified,
                locator_status: locator.clone(),
            };
            if let Err(error) = graph.evidence(evidence) {
                return Ok(refusal("evidence_validation", error.to_string()));
            }
            evidence_records.insert(id.to_string(), (last_modified, lineage, locator));
        }

        for (index, raw) in raw_edges.iter().enumerate() {
            let claim = match raw.get("claim").and_then(Value::as_str) {
                Some(claim) if !claim.trim().is_empty() && claim.len() <= MAX_ID_BYTES => claim,
                Some(_) => {
                    return Ok(refusal(
                        "edge_validation",
                        format!("edges[{index}].claim must contain 1 to {MAX_ID_BYTES} bytes"),
                    ));
                }
                None => {
                    return Ok(refusal(
                        "edge_validation",
                        format!("edges[{index}] requires a claim string"),
                    ));
                }
            };
            let evidence = match raw.get("evidence").and_then(Value::as_str) {
                Some(evidence) if !evidence.trim().is_empty() && evidence.len() <= MAX_ID_BYTES => {
                    evidence
                }
                Some(_) => {
                    return Ok(refusal(
                        "edge_validation",
                        format!("edges[{index}].evidence must contain 1 to {MAX_ID_BYTES} bytes"),
                    ));
                }
                None => {
                    return Ok(refusal(
                        "edge_validation",
                        format!("edges[{index}] requires an evidence string"),
                    ));
                }
            };
            let kind = match raw.get("kind") {
                Some(value) => match serde_json::from_value::<GroundingEdgeKind>(value.clone()) {
                    Ok(kind) => kind,
                    Err(error) => {
                        return Ok(refusal(
                            "edge_validation",
                            format!("edges[{index}].kind is invalid: {error}"),
                        ));
                    }
                },
                None => {
                    return Ok(refusal(
                        "edge_validation",
                        format!("edges[{index}] requires kind: supports, contradicts, or adjacent"),
                    ));
                }
            };
            if let Err(error) = graph.link(GroundingSupportEdge {
                claim: claim.to_string(),
                evidence: evidence.to_string(),
                kind,
            }) {
                return Ok(refusal("edge_validation", error.to_string()));
            }
        }

        let freeze = match arguments.get("stale_against") {
            None | Some(Value::Null) => None,
            Some(value) => {
                let raw = value
                    .as_str()
                    .ok_or("stale_against must be an RFC-3339 string")?;
                match FactoryTimestamp::parse(raw) {
                    Ok(timestamp) => Some(timestamp),
                    Err(error) => {
                        return Ok(refusal(
                            "staleness_validation",
                            format!("stale_against is invalid: {error}"),
                        ));
                    }
                }
            }
        };

        let state_name = |state: &GroundingClaimState| match state {
            GroundingClaimState::Supported => "supported",
            GroundingClaimState::Contested => "contested",
            GroundingClaimState::Contradicted => "contradicted",
            GroundingClaimState::Unsupported => "unsupported",
            GroundingClaimState::SupportUnverified => "support_unverified",
        };
        let kind_name = |kind: GroundingEdgeKind| match kind {
            GroundingEdgeKind::Supports => "supports",
            GroundingEdgeKind::Contradicts => "contradicts",
            GroundingEdgeKind::Adjacent => "adjacent",
        };
        let stale_ids: BTreeSet<String> = freeze
            .map(|timestamp| {
                graph
                    .stale_against(timestamp)
                    .into_iter()
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        let lineage_gap_ids: BTreeSet<String> = graph
            .lineage_gaps()
            .into_iter()
            .map(str::to_string)
            .collect();
        let mut claim_ids = graph
            .states()
            .keys()
            .map(|claim| (*claim).to_string())
            .collect::<Vec<_>>();
        claim_ids.sort();

        let all_claim_rows = claim_ids
            .iter()
            .map(|claim| {
                let state = graph.state(claim).expect("claim came from graph states");
                let mut supports = Vec::new();
                let mut contradictions = Vec::new();
                let mut adjacent = Vec::new();
                let mut resolved_supports = 0usize;
                for edge in graph.edges().iter().filter(|edge| edge.claim == *claim) {
                    match edge.kind {
                        GroundingEdgeKind::Supports => {
                            supports.push(edge.evidence.clone());
                            if evidence_records
                                .get(&edge.evidence)
                                .map(|(_, _, locator)| locator.is_resolved())
                                .unwrap_or(false)
                            {
                                resolved_supports += 1;
                            }
                        }
                        GroundingEdgeKind::Contradicts => contradictions.push(edge.evidence.clone()),
                        GroundingEdgeKind::Adjacent => adjacent.push(edge.evidence.clone()),
                    }
                }
                json!({
                    "id": claim,
                    "state": state_name(&state),
                    "grounded": state == GroundingClaimState::Supported,
                    "support_evidence_ids": supports,
                    "contradicting_evidence_ids": contradictions,
                    "adjacent_evidence_ids": adjacent,
                    "support_edge_count": supports.len(),
                    "resolved_support_edge_count": resolved_supports,
                    "unresolved_support_edge_count": supports.len().saturating_sub(resolved_supports),
                })
            })
            .collect::<Vec<_>>();

        let all_evidence_rows = evidence_records
            .iter()
            .map(|(id, (last_modified, lineage, locator))| {
                let related = graph.edges().iter().filter(|edge| edge.evidence == *id);
                let mut claims = BTreeSet::new();
                let mut support_edges = 0usize;
                let mut contradiction_edges = 0usize;
                let mut adjacent_edges = 0usize;
                for edge in related {
                    claims.insert(edge.claim.clone());
                    match edge.kind {
                        GroundingEdgeKind::Supports => support_edges += 1,
                        GroundingEdgeKind::Contradicts => contradiction_edges += 1,
                        GroundingEdgeKind::Adjacent => adjacent_edges += 1,
                    }
                }
                json!({
                    "id": id,
                    "last_modified": last_modified.to_rfc3339(),
                    "lineage": lineage,
                    "lineage_gap": lineage_gap_ids.contains(id),
                    "locator_status": serde_json::to_value(locator).expect("locator serializes"),
                    "resolved": locator.is_resolved(),
                    "stale": stale_ids.contains(id),
                    "linked_claim_ids": claims,
                    "edge_count": support_edges + contradiction_edges + adjacent_edges,
                    "support_edge_count": support_edges,
                    "contradiction_edge_count": contradiction_edges,
                    "adjacent_edge_count": adjacent_edges,
                    "orphan": claims.is_empty(),
                })
            })
            .collect::<Vec<_>>();

        let all_edge_rows = graph
            .edges()
            .iter()
            .enumerate()
            .map(|(index, edge)| {
                let locator_status = evidence_records
                    .get(&edge.evidence)
                    .map(|(_, _, locator)| locator)
                    .expect("graph edges have declared evidence");
                json!({
                    "index": index,
                    "claim": edge.claim,
                    "evidence": edge.evidence,
                    "kind": kind_name(edge.kind),
                    "locator_resolved": locator_status.is_resolved(),
                })
            })
            .collect::<Vec<_>>();

        let mut edge_multiplicity: BTreeMap<(String, String, GroundingEdgeKind), usize> =
            BTreeMap::new();
        for edge in graph.edges() {
            *edge_multiplicity
                .entry((edge.claim.clone(), edge.evidence.clone(), edge.kind))
                .or_default() += 1;
        }
        let duplicate_edge_count = edge_multiplicity
            .values()
            .map(|count| count.saturating_sub(1))
            .sum::<usize>();

        let contested_ids = claim_ids
            .iter()
            .filter(|claim| graph.state(claim) == Some(GroundingClaimState::Contested))
            .cloned()
            .collect::<BTreeSet<_>>();
        let contradicted_ids = claim_ids
            .iter()
            .filter(|claim| graph.state(claim) == Some(GroundingClaimState::Contradicted))
            .cloned()
            .collect::<BTreeSet<_>>();
        let unsupported_ids = claim_ids
            .iter()
            .filter(|claim| graph.state(claim) == Some(GroundingClaimState::Unsupported))
            .cloned()
            .collect::<BTreeSet<_>>();
        let unverified_ids = claim_ids
            .iter()
            .filter(|claim| graph.state(claim) == Some(GroundingClaimState::SupportUnverified))
            .cloned()
            .collect::<BTreeSet<_>>();
        let orphan_ids = evidence_records
            .keys()
            .filter(|id| !graph.edges().iter().any(|edge| edge.evidence == **id))
            .cloned()
            .collect::<BTreeSet<_>>();
        let bounded_ids = |ids: &BTreeSet<String>| {
            json!({
                "ids": ids.iter().take(max_items).cloned().collect::<Vec<_>>(),
                "total": ids.len(),
                "omitted": ids.len().saturating_sub(max_items),
            })
        };
        let bounded_rows = |rows: &[Value]| {
            json!({
                "rows": rows.iter().take(max_items).cloned().collect::<Vec<_>>(),
                "returned": rows.len().min(max_items),
                "total": rows.len(),
                "omitted": rows.len().saturating_sub(max_items),
            })
        };
        let census = graph.census();
        let resolved_count = evidence_records
            .values()
            .filter(|(_, _, locator)| locator.is_resolved())
            .count();
        let not_checked_count = evidence_records
            .values()
            .filter(|(_, _, locator)| matches!(locator, GroundingLocatorStatus::NotChecked))
            .count();
        let unresolvable_count = evidence_records
            .values()
            .filter(|(_, _, locator)| {
                matches!(locator, GroundingLocatorStatus::Unresolvable { .. })
            })
            .count();
        let support_edge_count = graph
            .edges()
            .iter()
            .filter(|edge| edge.kind == GroundingEdgeKind::Supports)
            .count();
        let contradiction_edge_count = graph
            .edges()
            .iter()
            .filter(|edge| edge.kind == GroundingEdgeKind::Contradicts)
            .count();

        Ok(json!({
            "ok": true,
            "schema": SCHEMA,
            "workflow": "bioeval_grounding_audit",
            "claims": bounded_rows(&all_claim_rows),
            "evidence": bounded_rows(&all_evidence_rows),
            "edges": bounded_rows(&all_edge_rows),
            "census": {
                "claims": census.claims(),
                "supported": census.supported,
                "contested": census.contested,
                "contradicted": census.contradicted,
                "unsupported": census.unsupported,
                "support_unverified": census.support_unverified,
                "adjacent_citations": census.adjacent_citations,
                "fully_grounded": census.fully_grounded(),
            },
            "graph": {
                "claim_count": claim_ids.len(),
                "evidence_count": evidence_records.len(),
                "edge_count": graph.edges().len(),
                "support_edge_count": support_edge_count,
                "contradiction_edge_count": contradiction_edge_count,
                "adjacent_edge_count": census.adjacent_citations,
                "duplicate_edge_count": duplicate_edge_count,
            },
            "locator_census": {
                "resolved": resolved_count,
                "not_checked": not_checked_count,
                "unresolvable": unresolvable_count,
            },
            "staleness": {
                "requested": freeze.is_some(),
                "freeze": freeze.map(|timestamp| timestamp.to_rfc3339()),
                "stale_count": stale_ids.len(),
                "stale_evidence": bounded_ids(&stale_ids),
            },
            "findings": {
                "contested_claims": bounded_ids(&contested_ids),
                "contradicted_claims": bounded_ids(&contradicted_ids),
                "unsupported_claims": bounded_ids(&unsupported_ids),
                "support_unverified_claims": bounded_ids(&unverified_ids),
                "lineage_gap_evidence": bounded_ids(&lineage_gap_ids),
                "orphan_evidence": bounded_ids(&orphan_ids),
                "adjacent_citation_count": census.adjacent_citations,
                "duplicate_edge_count": duplicate_edge_count,
            },
            "guarantees": [
                "claim states are the grounding kernel's five-way partition; support is not averaged into a citation percentage",
                "a claim with both support and contradiction remains contested even when supporting evidence is resolved",
                "not_checked and unresolvable locators are not promoted to resolved support",
                "staleness is reproducible because it is measured against the explicit freeze, never against the wall clock",
                "all bounded projections carry total and omitted counts so truncation cannot look like absence",
            ],
            "limitations": [
                "the route audits caller-declared atomic claims and evidence; it does not extract claims from prose",
                "locators and digests are recorded states only; no network, filesystem, or artifact dereference occurs",
                "lineage gaps identify empty recorded ancestry and do not establish biological provenance or assay compatibility",
                "a fully grounded graph is a provenance predicate, not biological truth, clinical validity, or causal identification",
            ],
        }))
    }

    pub(super) fn bioeval_estimand_audit(&self, arguments: &Value) -> Result<Value, String> {
        const SCHEMA: &str = "bioprism-mcp/bioeval-estimand-audit/0.1";
        const MAX_CORROBORATIONS: usize = 256;
        const MAX_TRANSPORT_REQUESTS: usize = 256;
        const MAX_TEXT_BYTES: usize = 4_096;

        let raw_estimand = arguments
            .get("estimand")
            .and_then(Value::as_object)
            .ok_or("estimand is required and must be an object")?;
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot measure estimand audit input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("estimand audit input exceeds the 20000000-byte safety bound".into());
        }
        let refusal = |stage: &str, detail: String| {
            json!({
                "ok": false,
                "schema": SCHEMA,
                "workflow": "bioeval_estimand_audit",
                "stage": stage,
                "refusal": detail,
                "fail_closed": true,
                "guarantees": [
                    "an incomplete five-element estimand never becomes a successful finding projection",
                    "model-conditional findings cannot be promoted by the same model or by an implicit boolean",
                    "identification declaration, identification probing, and identification validity remain separate",
                ],
                "limitations": [
                    "the route records identification assumptions and checks but does not run a causal graph or d-separation engine",
                    "corroboration is caller-declared external evidence and is not independently authenticated",
                    "transport checks only declared scope membership; scope mapping and loss accounting belong to bioprism-scope",
                ],
            })
        };
        let text_field = |name: &str| -> Result<String, String> {
            let value = raw_estimand
                .get(name)
                .and_then(Value::as_str)
                .ok_or_else(|| format!("estimand.{name} requires a string"))?;
            if value.trim().is_empty() || value.len() > MAX_TEXT_BYTES {
                return Err(format!(
                    "estimand.{name} must contain 1 to {MAX_TEXT_BYTES} bytes"
                ));
            }
            Ok(value.to_string())
        };
        let intervention = text_field("intervention")?;
        let comparator = text_field("comparator")?;
        let unit = text_field("unit")?;
        let outcome = text_field("outcome")?;
        let horizon = text_field("horizon")?;
        let scope = text_field("scope")?;
        let estimand =
            match BioevalEstimand::declare(intervention, comparator, unit, outcome, horizon, scope)
            {
                Ok(estimand) => estimand,
                Err(error) => return Ok(refusal("estimand_validation", error.to_string())),
            };

        let kind = match arguments.get("kind") {
            Some(value) => match serde_json::from_value::<BioevalClaimKind>(value.clone()) {
                Ok(kind) => kind,
                Err(error) => {
                    return Ok(refusal(
                        "kind_validation",
                        format!("kind is invalid: {error}"),
                    ));
                }
            },
            None => return Err("kind is required: association or intervention".into()),
        };
        let basis = match arguments.get("basis") {
            Some(value) => match serde_json::from_value::<BioevalEvidentiary>(value.clone()) {
                Ok(basis) => basis,
                Err(error) => {
                    return Ok(refusal(
                        "basis_validation",
                        format!("basis is invalid: {error}"),
                    ));
                }
            },
            None => return Err("basis is required and must carry an evidentiary tag".into()),
        };
        let basis_source = match &basis {
            BioevalEvidentiary::ModelConditional { model } => ("model_conditional", model.clone()),
            BioevalEvidentiary::Observational { dataset } => ("observational", dataset.clone()),
            BioevalEvidentiary::Experimental { study } => ("experimental", study.clone()),
        };
        if basis_source.1.trim().is_empty() || basis_source.1.len() > MAX_TEXT_BYTES {
            return Ok(refusal(
                "basis_validation",
                format!("basis source must contain 1 to {MAX_TEXT_BYTES} bytes"),
            ));
        }

        let identification = match arguments.get("identification") {
            None | Some(Value::Null) => BioevalIdentification::NotAssessed,
            Some(value) => match serde_json::from_value::<BioevalIdentification>(value.clone()) {
                Ok(identification) => identification,
                Err(error) => {
                    return Ok(refusal(
                        "identification_validation",
                        format!("identification is invalid: {error}"),
                    ));
                }
            },
        };
        let identification_summary = match &identification {
            BioevalIdentification::NotAssessed => json!({
                "status": "not_assessed",
                "strategy": Value::Null,
                "assumption_count": 0,
                "check_count": 0,
                "failed_check_count": 0,
                "probed": false,
            }),
            BioevalIdentification::Declared {
                strategy,
                assumptions,
            } => json!({
                "status": "declared",
                "strategy": strategy,
                "assumption_count": assumptions.len(),
                "check_count": 0,
                "failed_check_count": 0,
                "probed": false,
            }),
            BioevalIdentification::Probed {
                strategy,
                assumptions,
                checks,
            } => json!({
                "status": "probed",
                "strategy": strategy,
                "assumption_count": assumptions.len(),
                "check_count": checks.len(),
                "failed_check_count": checks.iter().filter(|check| !check.passed).count(),
                "failed_check_names": checks.iter().filter(|check| !check.passed).map(|check| check.name.clone()).collect::<Vec<_>>(),
                "probed": true,
            }),
        };
        if arguments
            .get("require_identification")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && matches!(identification, BioevalIdentification::NotAssessed)
        {
            return Ok(refusal(
                "identification_policy",
                "require_identification was true but identification is not assessed".into(),
            ));
        }

        let raw_corroborations = match arguments.get("corroborations") {
            None | Some(Value::Null) => Vec::new(),
            Some(value) => value
                .as_array()
                .ok_or("corroborations must be an array")?
                .clone(),
        };
        if raw_corroborations.len() > MAX_CORROBORATIONS {
            return Err(format!(
                "corroborations are bounded at {MAX_CORROBORATIONS} rows"
            ));
        }
        let mut corroborations = Vec::with_capacity(raw_corroborations.len());
        for (index, raw) in raw_corroborations.iter().enumerate() {
            let corroboration: BioevalCorroboration = match serde_json::from_value(raw.clone()) {
                Ok(corroboration) => corroboration,
                Err(error) => {
                    return Ok(refusal(
                        "corroboration_validation",
                        format!("corroborations[{index}] is invalid: {error}"),
                    ));
                }
            };
            if corroboration.source.trim().is_empty()
                || corroboration.source.len() > MAX_TEXT_BYTES
                || corroboration.detail.trim().is_empty()
                || corroboration.detail.len() > MAX_TEXT_BYTES
            {
                return Ok(refusal(
                    "corroboration_validation",
                    format!(
                        "corroborations[{index}] source and detail must contain 1 to {MAX_TEXT_BYTES} bytes"
                    ),
                ));
            }
            corroborations.push(corroboration);
        }
        if arguments
            .get("require_corroboration")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && matches!(&basis, BioevalEvidentiary::ModelConditional { .. })
            && corroborations.is_empty()
        {
            return Ok(refusal(
                "corroboration_policy",
                "require_corroboration was true but no external corroboration was supplied".into(),
            ));
        }

        let mut finding = BioevalFinding::new(estimand, kind, basis);
        finding = finding
            .identified_by(identification)
            .map_err(|error| format!("identification validation failed: {error}"))?;
        for (index, corroboration) in corroborations.iter().cloned().enumerate() {
            if let Err(error) = finding.promote(corroboration) {
                return Ok(refusal(
                    "corroboration_validation",
                    format!("corroborations[{index}] was refused: {error}"),
                ));
            }
        }

        let raw_transport = match arguments.get("transport_requests") {
            None | Some(Value::Null) => Vec::new(),
            Some(value) => value
                .as_array()
                .ok_or("transport_requests must be an array")?
                .clone(),
        };
        if raw_transport.len() > MAX_TRANSPORT_REQUESTS {
            return Err(format!(
                "transport_requests are bounded at {MAX_TRANSPORT_REQUESTS} rows"
            ));
        }
        let mut transport_requests = Vec::with_capacity(raw_transport.len());
        let mut transport_targets = BTreeSet::new();
        for (index, raw) in raw_transport.iter().enumerate() {
            let target = raw
                .get("target")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("transport_requests[{index}] requires target"))?;
            if target.trim().is_empty() || target.len() > MAX_TEXT_BYTES {
                return Ok(refusal(
                    "transport_validation",
                    format!(
                        "transport_requests[{index}].target must contain 1 to {MAX_TEXT_BYTES} bytes"
                    ),
                ));
            }
            if !transport_targets.insert(target.to_string()) {
                return Ok(refusal(
                    "transport_validation",
                    format!("transport_requests[{index}] duplicates target {target:?}"),
                ));
            }
            let declared = raw
                .get("declared_scopes")
                .ok_or_else(|| format!("transport_requests[{index}] requires declared_scopes"))?
                .as_array()
                .ok_or_else(|| {
                    format!("transport_requests[{index}].declared_scopes must be an array")
                })?;
            let mut declared_scopes = BTreeSet::new();
            for (scope_index, scope) in declared.iter().enumerate() {
                let scope = scope.as_str().ok_or_else(|| {
                    format!("transport_requests[{index}].declared_scopes[{scope_index}] must be a string")
                })?;
                if scope.trim().is_empty() || scope.len() > MAX_TEXT_BYTES {
                    return Ok(refusal(
                        "transport_validation",
                        format!(
                            "transport_requests[{index}].declared_scopes[{scope_index}] is invalid"
                        ),
                    ));
                }
                declared_scopes.insert(scope.to_string());
            }
            transport_requests.push((target.to_string(), declared_scopes));
        }
        let mut transport_rows = Vec::with_capacity(transport_requests.len());
        let mut transport_refusals = 0usize;
        for (target, declared_scopes) in &transport_requests {
            match finding.estimand.transport_to(target, declared_scopes) {
                Ok(()) => transport_rows.push(json!({
                    "target": target,
                    "declared_scopes": declared_scopes,
                    "ok": true,
                    "refusal": Value::Null,
                })),
                Err(error) => {
                    transport_refusals += 1;
                    transport_rows.push(json!({
                        "target": target,
                        "declared_scopes": declared_scopes,
                        "ok": false,
                        "refusal": error.to_string(),
                    }));
                }
            }
        }
        if arguments
            .get("strict_transport")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && transport_refusals > 0
        {
            return Ok(refusal(
                "transport_policy",
                format!("strict_transport refused {transport_refusals} out-of-scope request(s)"),
            ));
        }

        let claim_kind_name = match finding.kind {
            BioevalClaimKind::Association => "association",
            BioevalClaimKind::Intervention => "intervention",
        };
        let transport_status = if transport_requests.is_empty() {
            "not_requested"
        } else if transport_refusals == 0 {
            "all_declared"
        } else if transport_refusals == transport_requests.len() {
            "all_refused"
        } else {
            "partially_declared"
        };
        let identification_value = serde_json::to_value(&finding.identification)
            .map_err(|error| format!("cannot serialize identification: {error}"))?;
        let basis_value = serde_json::to_value(&finding.basis)
            .map_err(|error| format!("cannot serialize evidentiary basis: {error}"))?;
        let corroborations_value = serde_json::to_value(finding.corroborations())
            .map_err(|error| format!("cannot serialize corroborations: {error}"))?;
        Ok(json!({
            "ok": true,
            "schema": SCHEMA,
            "workflow": "bioeval_estimand_audit",
            "estimand": {
                "intervention": finding.estimand.intervention(),
                "comparator": finding.estimand.comparator(),
                "unit": finding.estimand.unit(),
                "outcome": finding.estimand.outcome(),
                "horizon": finding.estimand.horizon(),
                "scope": finding.estimand.scope(),
                "five_elements_complete": true,
            },
            "claim": {
                "kind": claim_kind_name,
                "basis": basis_value,
                "basis_kind": basis_source.0,
                "basis_source": basis_source.1,
                "identification": identification_value,
                "identification_summary": identification_summary,
                "corroborations": corroborations_value,
                "corroboration_count": finding.corroborations().len(),
                "still_model_conditional": finding.still_model_conditional(),
                "claim_language": finding.claim_language(),
            },
            "policies": {
                "require_identification": arguments.get("require_identification").and_then(Value::as_bool).unwrap_or(false),
                "require_corroboration": arguments.get("require_corroboration").and_then(Value::as_bool).unwrap_or(false),
                "strict_transport": arguments.get("strict_transport").and_then(Value::as_bool).unwrap_or(false),
            },
            "transport": {
                "status": transport_status,
                "requested": transport_requests.len(),
                "accepted": transport_requests.len().saturating_sub(transport_refusals),
                "refused": transport_refusals,
                "rows": transport_rows,
            },
            "guarantees": [
                "intervention, comparator, unit, outcome, horizon, and scope are declared through the real estimand constructor",
                "association findings render association language and cannot be rendered as intervention claims",
                "model-conditional findings retain their qualifier until a named external corroboration is accepted",
                "a same-model corroboration is refused rather than counted as independent promotion",
                "identification is reported as not_assessed, declared, or probed without claiming that assumptions hold",
                "out-of-scope transport remains a row-level refusal unless strict_transport explicitly requests a fail-closed gate",
            ],
            "limitations": [
                "the route does not construct a causal graph, run d-separation, estimate effects, simulate bias, or measure decision regret",
                "named assumptions and negative-control outcomes are caller-supplied records, not executed checks",
                "external corroboration is recorded provenance and does not establish biological or clinical truth",
                "scope membership is not a transport map; loss, comparability, and target-population evidence remain outside this kernel",
            ],
        }))
    }

    pub(super) fn bioeval_evaluator_audit(&self, arguments: &Value) -> Result<Value, String> {
        const SCHEMA: &str = "bioprism-mcp/bioeval-evaluator-audit/0.1";
        const MAX_RUNS: usize = 1_024;
        const MAX_ID_BYTES: usize = 256;

        let raw_runs = arguments
            .get("runs")
            .and_then(Value::as_array)
            .ok_or("runs is required and must be an array of serialized EvaluatorRun values")?;
        if raw_runs.len() > MAX_RUNS {
            return Err("runs are bounded at 1024 evaluator rows".into());
        }
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot measure evaluator audit input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("evaluator audit input exceeds the 20000000-byte safety bound".into());
        }
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if !(1..=1_000).contains(&max_items) {
            return Err("max_items must be between 1 and 1000".into());
        }
        let max_items = max_items as usize;
        let refusal = |stage: &str, detail: String| {
            json!({
                "ok": false,
                "schema": SCHEMA,
                "workflow": "bioeval_evaluator_audit",
                "stage": stage,
                "refusal": detail,
                "fail_closed": true,
                "guarantees": [
                    "an unhealthy evaluator cannot be projected as task evidence",
                    "healthy NotMet outcomes without diagnostics remain refused by the evaluator kernel",
                    "a panel with no usable task outcome never becomes a task score",
                ],
                "limitations": [
                    "the route does not execute commands, sandbox evaluators, mount fixtures, or mediate hidden-data access",
                    "diagnostics and health states are caller-supplied run records",
                    "hidden-data access is retained as a separate review finding and is not adjudicated by this route",
                ],
            })
        };

        let mut panel = BioevalEvaluatorPanel::new();
        let mut runs = Vec::with_capacity(raw_runs.len());
        for (index, raw) in raw_runs.iter().enumerate() {
            let run: BioevalEvaluatorRun = match serde_json::from_value(raw.clone()) {
                Ok(run) => run,
                Err(error) => {
                    return Ok(refusal(
                        "run_validation",
                        format!("runs[{index}] is not a valid EvaluatorRun: {error}"),
                    ));
                }
            };
            if run.evaluator.trim().is_empty() || run.evaluator.len() > MAX_ID_BYTES {
                return Ok(refusal(
                    "run_validation",
                    format!("runs[{index}].evaluator must contain 1 to {MAX_ID_BYTES} bytes"),
                ));
            }
            if let Err(error) = panel.record(run.clone()) {
                return Ok(refusal("run_validation", error.to_string()));
            }
            runs.push(run);
        }

        let bounded_strings = |values: &[String]| {
            json!({
                "items": values.iter().take(max_items).cloned().collect::<Vec<_>>(),
                "total": values.len(),
                "omitted": values.len().saturating_sub(max_items),
            })
        };
        let task_outcome_name = |outcome: BioevalTaskOutcome| match outcome {
            BioevalTaskOutcome::Met => "met",
            BioevalTaskOutcome::NotMet => "not_met",
            BioevalTaskOutcome::Inapplicable => "inapplicable",
        };
        let mut all_rows = Vec::with_capacity(runs.len());
        let mut unhealthy_ids = BTreeSet::new();
        let mut refused_ids = BTreeSet::new();
        let mut hidden_ids = BTreeSet::new();
        let mut duplicate_counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut met_count = 0usize;
        let mut not_met_count = 0usize;
        let mut inapplicable_count = 0usize;
        let mut task_evidence_count = 0usize;
        for (index, run) in runs.iter().enumerate() {
            *duplicate_counts.entry(run.evaluator.clone()).or_default() += 1;
            if !run.health.is_healthy() {
                unhealthy_ids.insert(run.evaluator.clone());
            }
            if run.hidden_data_touched() {
                hidden_ids.insert(run.evaluator.clone());
            }
            let task_outcome = run.task_outcome();
            let (outcome_value, outcome_status, outcome_refusal) = match task_outcome {
                Ok(outcome) => {
                    task_evidence_count += 1;
                    match outcome {
                        BioevalTaskOutcome::Met => met_count += 1,
                        BioevalTaskOutcome::NotMet => not_met_count += 1,
                        BioevalTaskOutcome::Inapplicable => inapplicable_count += 1,
                    }
                    (json!(task_outcome_name(outcome)), "accepted", Value::Null)
                }
                Err(error) => {
                    refused_ids.insert(run.evaluator.clone());
                    (Value::Null, "refused", json!(error.to_string()))
                }
            };
            let diagnostic = json!({
                "command": run.diagnostic.command,
                "exit_state": run.diagnostic.exit_state,
                "diff": run.diagnostic.diff,
                "logs": bounded_strings(&run.diagnostic.logs),
                "hidden_data_access": bounded_strings(&run.diagnostic.hidden_data_access),
                "empty": run.diagnostic.is_empty(),
            });
            all_rows.push(json!({
                "index": index,
                "evaluator": run.evaluator,
                "health": serde_json::to_value(&run.health).expect("health serializes"),
                "health_label": run.health.label(),
                "healthy": run.health.is_healthy(),
                "reached": serde_json::to_value(run.reached).expect("task outcome serializes"),
                "task_outcome": outcome_value,
                "task_outcome_status": outcome_status,
                "task_outcome_refusal": outcome_refusal,
                "unscored_reason": serde_json::to_value(run.unscored_reason()).expect("unscored reason serializes"),
                "hidden_data_touched": run.hidden_data_touched(),
                "diagnostic": diagnostic,
            }));
        }
        let duplicate_ids = duplicate_counts
            .iter()
            .filter(|(_, count)| **count > 1)
            .map(|(id, _)| id.clone())
            .collect::<BTreeSet<_>>();
        let no_task_evidence = !panel.says_anything();
        if arguments
            .get("require_task_evidence")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && no_task_evidence
        {
            return Ok(refusal(
                "panel_policy",
                "require_task_evidence was true but no evaluator produced a usable task outcome"
                    .into(),
            ));
        }
        if arguments
            .get("fail_on_hidden_data")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && !hidden_ids.is_empty()
        {
            return Ok(refusal(
                "hidden_data_policy",
                format!(
                    "fail_on_hidden_data refused {} evaluator row(s)",
                    hidden_ids.len()
                ),
            ));
        }
        let bounded_rows = |rows: &[Value]| {
            json!({
                "rows": rows.iter().take(max_items).cloned().collect::<Vec<_>>(),
                "returned": rows.len().min(max_items),
                "total": rows.len(),
                "omitted": rows.len().saturating_sub(max_items),
            })
        };
        let bounded_ids = |ids: &BTreeSet<String>, limit: usize| {
            json!({
                "ids": ids.iter().take(limit).cloned().collect::<Vec<_>>(),
                "total": ids.len(),
                "omitted": ids.len().saturating_sub(limit),
            })
        };
        let duplicate_count = duplicate_ids.len();
        let posture = if no_task_evidence {
            "no_task_evidence"
        } else if !hidden_ids.is_empty() {
            "review_required_hidden_data"
        } else {
            "task_evidence_available"
        };
        Ok(json!({
            "ok": true,
            "schema": SCHEMA,
            "workflow": "bioeval_evaluator_audit",
            "runs": bounded_rows(&all_rows),
            "panel": {
                "run_count": runs.len(),
                "healthy_count": runs.iter().filter(|run| run.health.is_healthy()).count(),
                "unhealthy_count": runs.iter().filter(|run| !run.health.is_healthy()).count(),
                "task_evidence_count": task_evidence_count,
                "refused_task_outcome_count": refused_ids.len(),
                "says_anything": panel.says_anything(),
                "no_task_evidence": no_task_evidence,
                "hidden_data_touched_count": hidden_ids.len(),
                "duplicate_evaluator_count": duplicate_count,
                "outcomes": {
                    "met": met_count,
                    "not_met": not_met_count,
                    "inapplicable": inapplicable_count,
                },
                "posture": posture,
            },
            "findings": {
                "unhealthy_evaluators": bounded_ids(&unhealthy_ids, max_items),
                "refused_task_outcomes": bounded_ids(&refused_ids, max_items),
                "hidden_data_evaluators": bounded_ids(&hidden_ids, max_items),
                "duplicate_evaluator_ids": bounded_ids(&duplicate_ids, max_items),
                "unscored_evaluator_count": unhealthy_ids.len(),
            },
            "guarantees": [
                "timed out, errored, and fixture-broken evaluators remain unscored rather than becoming task failures",
                "healthy NotMet rows require diagnostic evidence through the real evaluator kernel",
                "Met, NotMet, and Inapplicable are counted only after the evaluator was healthy enough to produce a task outcome",
                "hidden-data access, task outcome, evaluator health, and diagnostic completeness remain separate fields",
                "bounded rows and identifier findings retain total and omitted counts",
            ],
            "limitations": [
                "the route audits caller-supplied evaluator records and never executes a command or creates a sandbox",
                "fixture integrity, hidden-file access, logs, and exit states are not independently verified",
                "a usable task outcome is evaluation evidence, not biological truth, clinical validity, or release approval",
                "duplicate evaluator identifiers are surfaced as a finding but are not silently merged or majority-voted",
            ],
        }))
    }

    pub(super) fn bioeval_metamorphic_audit(&self, arguments: &Value) -> Result<Value, String> {
        const SCHEMA: &str = "bioprism-mcp/bioeval-metamorphic-audit/0.1";
        const MAX_FAMILIES: usize = 1_024;
        const MAX_TRIALS: usize = 4_096;
        const MAX_ID_BYTES: usize = 256;

        let raw_families = arguments.get("families").and_then(Value::as_array).ok_or(
            "families is required and must be an array of serialized metamorphic Family values",
        )?;
        if raw_families.is_empty() || raw_families.len() > MAX_FAMILIES {
            return Err("families must contain 1 to 1024 family rows".into());
        }
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot measure metamorphic audit input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("metamorphic audit input exceeds the 20000000-byte safety bound".into());
        }
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if !(1..=1_000).contains(&max_items) {
            return Err("max_items must be between 1 and 1000".into());
        }
        let max_items = max_items as usize;
        let refusal = |stage: &str, detail: String| {
            json!({
                "ok": false,
                "schema": SCHEMA,
                "workflow": "bioeval_metamorphic_audit",
                "stage": stage,
                "refusal": detail,
                "fail_closed": true,
                "guarantees": [
                    "false sensitivity, false invariance, wrong direction, and undetermined remain distinct",
                    "undetermined trials never become consistent trials",
                    "a family relation mismatch or duplicate trial cannot be averaged into a suite result",
                ],
                "limitations": [
                    "the route audits caller-supplied trial responses and does not generate mutations or run a system",
                    "the route does not infer mutation-family coverage from a registry",
                ],
            })
        };
        let bounded_ids = |ids: &BTreeSet<String>| {
            json!({
                "ids": ids.iter().take(max_items).cloned().collect::<Vec<_>>(),
                "total": ids.len(),
                "omitted": ids.len().saturating_sub(max_items),
            })
        };

        let mut suite = BioevalMetamorphicSuite::new();
        let mut families = Vec::with_capacity(raw_families.len());
        let mut family_ids = BTreeSet::new();
        let mut trial_total = 0usize;
        for (index, raw) in raw_families.iter().enumerate() {
            let parsed: BioevalMetamorphicFamily = match serde_json::from_value(raw.clone()) {
                Ok(family) => family,
                Err(error) => {
                    return Ok(refusal(
                        "family_deserialization",
                        format!("families[{index}] is not a valid metamorphic Family: {error}"),
                    ));
                }
            };
            if parsed.id.trim().is_empty() || parsed.id.len() > MAX_ID_BYTES {
                return Ok(refusal(
                    "family_validation",
                    format!("families[{index}].id must contain 1 to {MAX_ID_BYTES} bytes"),
                ));
            }
            if !family_ids.insert(parsed.id.clone()) {
                return Ok(refusal(
                    "family_validation",
                    format!("family {:?} appears more than once", parsed.id),
                ));
            }
            if parsed.trials().is_empty() || parsed.trials().len() > MAX_TRIALS {
                return Ok(refusal(
                    "family_validation",
                    format!(
                        "family {:?} must contain 1 to {MAX_TRIALS} trials",
                        parsed.id
                    ),
                ));
            }
            trial_total = trial_total.saturating_add(parsed.trials().len());
            if trial_total > MAX_TRIALS {
                return Ok(refusal(
                    "suite_validation",
                    format!("suite trials are bounded at {MAX_TRIALS} rows"),
                ));
            }
            let mut normalized = BioevalMetamorphicFamily::declaring(&parsed.id, parsed.relation);
            for trial in parsed.trials() {
                if trial.id.trim().is_empty() || trial.id.len() > MAX_ID_BYTES {
                    return Ok(refusal(
                        "trial_validation",
                        format!(
                            "family {:?} contains a trial id outside the 1 to {MAX_ID_BYTES}-byte bound",
                            parsed.id
                        ),
                    ));
                }
                if let Err(error) = normalized.record(trial.clone()) {
                    return Ok(refusal("trial_validation", error.to_string()));
                }
            }
            if let Err(error) = suite.add(normalized.clone()) {
                return Ok(refusal("family_validation", error.to_string()));
            }
            families.push(normalized);
        }

        let reports = match suite.reports() {
            Ok(reports) => reports,
            Err(error) => return Ok(refusal("family_report", error.to_string())),
        };
        let has_invariant = suite
            .relations_covered()
            .contains(&BioevalMetamorphicRelation::Invariant);
        let has_directional = suite.relations_covered().iter().any(|relation| {
            matches!(
                relation,
                BioevalMetamorphicRelation::DirectionalChange { .. }
            )
        });
        if arguments
            .get("require_both_relations")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && !(has_invariant && has_directional)
        {
            return Ok(refusal(
                "relation_coverage",
                "require_both_relations was true but the suite does not cover invariant and directional-change families".into(),
            ));
        }

        let mut rows = Vec::with_capacity(families.len());
        let mut failing_families = BTreeSet::new();
        let mut undetermined_families = BTreeSet::new();
        let mut false_sensitivity_trials = BTreeSet::new();
        let mut false_invariance_trials = BTreeSet::new();
        let mut wrong_direction_trials = BTreeSet::new();
        let mut undetermined_count = 0usize;
        for (family, report) in families.iter().zip(reports.iter()) {
            if !report.witnesses.is_empty() {
                failing_families.insert(family.id.clone());
            }
            if report.undetermined > 0 {
                undetermined_families.insert(family.id.clone());
                undetermined_count += report.undetermined;
            }
            for trial in family.trials() {
                match bioeval_metamorphic_verdict(trial.relation, trial.response) {
                    BioevalTrialVerdict::FalseSensitivity => {
                        false_sensitivity_trials.insert(trial.id.clone());
                    }
                    BioevalTrialVerdict::FalseInvariance => {
                        false_invariance_trials.insert(trial.id.clone());
                    }
                    BioevalTrialVerdict::WrongDirection => {
                        wrong_direction_trials.insert(trial.id.clone());
                    }
                    BioevalTrialVerdict::Consistent | BioevalTrialVerdict::Undetermined => {}
                }
            }
            let trial_rows = family
                .trials()
                .iter()
                .take(max_items)
                .map(|trial: &BioevalMetamorphicTrial| {
                    json!({
                        "id": trial.id,
                        "relation": trial.relation,
                        "response": trial.response,
                        "verdict": bioeval_metamorphic_verdict(trial.relation, trial.response),
                        "evidence": bioeval_metamorphic_verdict(trial.relation, trial.response).is_evidence(),
                        "consistent": bioeval_metamorphic_verdict(trial.relation, trial.response).is_consistent(),
                    })
                })
                .collect::<Vec<_>>();
            rows.push(json!({
                "id": family.id,
                "relation": family.relation,
                "trial_count": family.trials().len(),
                "trials": {
                    "rows": trial_rows,
                    "returned": family.trials().len().min(max_items),
                    "total": family.trials().len(),
                    "omitted": family.trials().len().saturating_sub(max_items),
                },
                "consistent": report.consistent,
                "false_sensitivity": report.false_sensitivity,
                "false_invariance": report.false_invariance,
                "wrong_direction": report.wrong_direction,
                "undetermined": report.undetermined,
                "evidential": report.evidential(),
                "consistency": report.consistency(),
                "witnesses": bounded_ids(&report.witnesses.iter().cloned().collect()),
            }));
        }
        if arguments
            .get("fail_on_undetermined")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && undetermined_count > 0
        {
            return Ok(refusal(
                "oracle_quality",
                format!(
                    "fail_on_undetermined refused {undetermined_count} trial(s) without comparable responses"
                ),
            ));
        }
        Ok(json!({
            "ok": true,
            "schema": SCHEMA,
            "workflow": "bioeval_metamorphic_audit",
            "suite": {
                "family_count": families.len(),
                "trial_count": trial_total,
                "relation_coverage": {
                    "invariant": has_invariant,
                    "directional_change": has_directional,
                    "complete": has_invariant && has_directional,
                },
                "failing_family_count": failing_families.len(),
                "undetermined_trial_count": undetermined_count,
                "has_suite_wide_consistency": false,
            },
            "families": {
                "rows": rows.into_iter().take(max_items).collect::<Vec<_>>(),
                "returned": families.len().min(max_items),
                "total": families.len(),
                "omitted": families.len().saturating_sub(max_items),
            },
            "findings": {
                "failing_families": bounded_ids(&failing_families),
                "undetermined_families": bounded_ids(&undetermined_families),
                "false_sensitivity_trials": bounded_ids(&false_sensitivity_trials),
                "false_invariance_trials": bounded_ids(&false_invariance_trials),
                "wrong_direction_trials": bounded_ids(&wrong_direction_trials),
            },
            "guarantees": [
                "false sensitivity and false invariance remain separate failure directions",
                "wrong direction is distinct from a response that did not move",
                "incomparable responses become undetermined and never enter the consistency denominator",
                "consistency is reported per family over evidential trials only",
                "the suite never emits a single cross-family consistency percentage",
                "bounded family, trial, witness, and finding projections retain total and omitted counts",
            ],
            "limitations": [
                "the route does not generate mutations, execute a system, or verify that the declared biology changed",
                "trial responses and direction labels are caller-supplied records",
                "family identifiers do not establish independent biological evidence; registry lineage remains external",
                "a consistent metamorphic response is robustness evidence, not biological or clinical validity",
            ],
        }))
    }

    pub(super) fn bioeval_waiver_audit(&self, arguments: &Value) -> Result<Value, String> {
        const SCHEMA: &str = "bioprism-mcp/bioeval-waiver-audit/0.1";
        const MAX_GATES: usize = 1_024;
        const MAX_WAIVERS: usize = 1_024;
        const MAX_ID_BYTES: usize = 256;

        let refusal = |stage: &str, detail: String| {
            json!({
                "ok": false,
                "schema": SCHEMA,
                "workflow": "bioeval_waiver_audit",
                "stage": stage,
                "refusal": detail,
                "fail_closed": true,
                "guarantees": [
                    "a safety veto can never be waived",
                    "an applied waiver preserves the underlying gate verdict",
                    "an expired or version-mismatched waiver never makes a release releasable",
                    "unevaluable gates remain distinct from violated gates",
                ],
                "limitations": [
                    "the route records caller-supplied gate verdicts and does not evaluate metrics, posteriors, or safety policy",
                    "authoriser identity and authority are assertions; no identity provider or signature verifier is invoked",
                    "the route does not publish a CI comment, deploy an artifact, or mutate an external release system",
                ],
            })
        };
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot measure waiver audit input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("waiver audit input exceeds the 20000000-byte safety bound".into());
        }
        let version = arguments
            .get("version")
            .and_then(Value::as_str)
            .ok_or("version is required and must be a release version string")?;
        if version.trim().is_empty() || version.len() > MAX_ID_BYTES {
            return Ok(refusal(
                "release_validation",
                format!("version must contain 1 to {MAX_ID_BYTES} bytes"),
            ));
        }
        let at_text = arguments
            .get("at")
            .and_then(Value::as_str)
            .ok_or("at is required and must be an RFC-3339 evaluation instant")?;
        let at = match FactoryTimestamp::parse(at_text) {
            Ok(at) => at,
            Err(error) => {
                return Ok(refusal(
                    "release_validation",
                    format!("at is not a valid RFC-3339 timestamp: {error}"),
                ));
            }
        };
        let raw_gates = arguments
            .get("gates")
            .and_then(Value::as_array)
            .ok_or("gates is required and must be an array of serialized Gate values")?;
        if raw_gates.is_empty() || raw_gates.len() > MAX_GATES {
            return Err("gates must contain 1 to 1024 release-gate rows".into());
        }
        let raw_waivers = arguments
            .get("waivers")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if raw_waivers.len() > MAX_WAIVERS {
            return Err("waivers must contain at most 1024 waiver rows".into());
        }
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if !(1..=1_000).contains(&max_items) {
            return Err("max_items must be between 1 and 1000".into());
        }
        let max_items = max_items as usize;

        let mut gates = Vec::with_capacity(raw_gates.len());
        let mut gate_ids = BTreeSet::new();
        for (index, raw) in raw_gates.iter().enumerate() {
            let gate: BioevalReleaseGate = match serde_json::from_value(raw.clone()) {
                Ok(gate) => gate,
                Err(error) => {
                    return Ok(refusal(
                        "gate_deserialization",
                        format!("gates[{index}] is not a valid Gate: {error}"),
                    ));
                }
            };
            if gate.id.trim().is_empty() || gate.id.len() > MAX_ID_BYTES {
                return Ok(refusal(
                    "gate_validation",
                    format!("gates[{index}].id must contain 1 to {MAX_ID_BYTES} bytes"),
                ));
            }
            if !gate_ids.insert(gate.id.clone()) {
                return Ok(refusal(
                    "gate_validation",
                    format!("gate {:?} appears more than once", gate.id),
                ));
            }
            gates.push(gate);
        }

        let mut decision = BioevalReleaseDecision::for_version(version, gates);
        let before_blocking = decision
            .blocking()
            .iter()
            .map(|gate| gate.id.clone())
            .collect::<BTreeSet<_>>();
        let mut waiver_ids = BTreeSet::new();
        for (index, raw) in raw_waivers.iter().enumerate() {
            let object = match raw.as_object() {
                Some(object) => object,
                None => {
                    return Ok(refusal(
                        "waiver_deserialization",
                        format!("waivers[{index}] must be an object"),
                    ));
                }
            };
            let gate = object
                .get("gate")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("waivers[{index}].gate is required"))?;
            if gate.trim().is_empty() || gate.len() > MAX_ID_BYTES {
                return Ok(refusal(
                    "waiver_validation",
                    format!("waivers[{index}].gate must contain 1 to {MAX_ID_BYTES} bytes"),
                ));
            }
            if !waiver_ids.insert(gate.to_string()) {
                return Ok(refusal(
                    "waiver_validation",
                    format!("more than one waiver names gate {:?}", gate),
                ));
            }
            let authoriser = object
                .get("authoriser")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("waivers[{index}].authoriser is required"))?;
            let rationale = object
                .get("rationale")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("waivers[{index}].rationale is required"))?;
            let expiry_text = object
                .get("expiry")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("waivers[{index}].expiry is required"))?;
            let expiry = match FactoryTimestamp::parse(expiry_text) {
                Ok(expiry) => expiry,
                Err(error) => {
                    return Ok(refusal(
                        "waiver_validation",
                        format!(
                            "waivers[{index}].expiry is not a valid RFC-3339 timestamp: {error}"
                        ),
                    ));
                }
            };
            let affected_versions = object
                .get("affected_versions")
                .cloned()
                .ok_or_else(|| format!("waivers[{index}].affected_versions is required"))
                .and_then(|value| {
                    serde_json::from_value::<Vec<String>>(value)
                        .map_err(|error| format!("waivers[{index}].affected_versions must be an array of strings: {error}"))
                })?;
            let follow_up = object
                .get("follow_up")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("waivers[{index}].follow_up is required"))?;
            let waiver = match BioevalReleaseWaiver::sign(
                gate,
                authoriser,
                rationale,
                expiry,
                affected_versions,
                follow_up,
            ) {
                Ok(waiver) => waiver,
                Err(error) => {
                    return Ok(refusal(
                        "waiver_validation",
                        format!("waivers[{index}] is incomplete: {error}"),
                    ));
                }
            };
            if let Err(error) = decision.waive(waiver, at) {
                return Ok(refusal(
                    "waiver_application",
                    format!("waivers[{index}] could not be applied: {error}"),
                ));
            }
        }

        let waived_ids = decision
            .waivers()
            .iter()
            .map(|waived| waived.gate.id.clone())
            .collect::<BTreeSet<_>>();
        let after_blocking = decision
            .blocking()
            .iter()
            .map(|gate| gate.id.clone())
            .collect::<BTreeSet<_>>();
        let unevaluable_ids = decision
            .unevaluable()
            .iter()
            .map(|gate| gate.id.clone())
            .collect::<BTreeSet<_>>();
        let safety_veto_ids = decision
            .gates()
            .iter()
            .filter(|gate| matches!(gate.kind, bioprism_bioevalx::waiver::GateKind::SafetyVeto))
            .map(|gate| gate.id.clone())
            .collect::<BTreeSet<_>>();
        let bounded_ids = |ids: &BTreeSet<String>| {
            json!({
                "ids": ids.iter().take(max_items).cloned().collect::<Vec<_>>(),
                "total": ids.len(),
                "omitted": ids.len().saturating_sub(max_items),
            })
        };
        let gate_rows = decision
            .gates()
            .iter()
            .take(max_items)
            .map(|gate| {
                json!({
                    "id": gate.id,
                    "kind": gate.kind,
                    "verdict": gate.verdict,
                    "blocks_before": before_blocking.contains(&gate.id),
                    "waived": waived_ids.contains(&gate.id),
                    "blocks_after": after_blocking.contains(&gate.id),
                    "unevaluable": matches!(gate.verdict, bioprism_bioevalx::waiver::GateVerdict::Unevaluable { .. }),
                })
            })
            .collect::<Vec<_>>();
        let waiver_rows = decision
            .waivers()
            .iter()
            .take(max_items)
            .map(|waived| {
                json!({
                    "gate": waived.gate.id,
                    "kind": waived.gate.kind,
                    "underlying_verdict": waived.gate.verdict,
                    "waiver": waived.waiver,
                    "applied_at": waived.applied_at,
                })
            })
            .collect::<Vec<_>>();
        let require_releasable = arguments
            .get("require_releasable")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let require_no_unevaluable = arguments
            .get("require_no_unevaluable")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if require_no_unevaluable && !unevaluable_ids.is_empty() {
            return Ok(refusal(
                "unknown_rate_policy",
                format!(
                    "require_no_unevaluable was true but {} gate(s) remain unevaluable",
                    unevaluable_ids.len()
                ),
            ));
        }
        if require_releasable && !decision.releasable() {
            return Ok(refusal(
                "release_gate_policy",
                format!(
                    "require_releasable was true but {} blocking gate(s) remain",
                    after_blocking.len()
                ),
            ));
        }
        Ok(json!({
            "ok": true,
            "schema": SCHEMA,
            "workflow": "bioeval_waiver_audit",
            "release": {
                "version": version,
                "evaluated_at": at,
                "gate_count": decision.gates().len(),
                "blocking_before": before_blocking.len(),
                "blocking_after": after_blocking.len(),
                "waived_count": decision.waivers().len(),
                "unevaluable_count": unevaluable_ids.len(),
                "releasable": decision.releasable(),
            },
            "gates": {
                "rows": gate_rows,
                "returned": decision.gates().len().min(max_items),
                "total": decision.gates().len(),
                "omitted": decision.gates().len().saturating_sub(max_items),
            },
            "waivers": {
                "rows": waiver_rows,
                "returned": decision.waivers().len().min(max_items),
                "total": decision.waivers().len(),
                "omitted": decision.waivers().len().saturating_sub(max_items),
            },
            "findings": {
                "still_blocking": bounded_ids(&after_blocking),
                "waived_gates": bounded_ids(&waived_ids),
                "unevaluable_gates": bounded_ids(&unevaluable_ids),
                "safety_vetoes": bounded_ids(&safety_veto_ids),
            },
            "guarantees": [
                "a waiver changes blocking posture only; the original gate verdict remains visible",
                "safety vetoes are never waivable, including when a waiver names the veto",
                "expired waivers and waivers for another release version are refused at application",
                "unevaluable gates remain separately countable for maximum-unknown-rate policy",
                "authoriser, rationale, expiry, affected versions, and follow-up are retained in every applied waiver",
                "bounded gate, waiver, and finding projections retain total and omitted counts",
            ],
            "limitations": [
                "the route does not calculate the gate verdict or verify the authoriser's identity or authority",
                "a releasable local decision is not a cryptographic signature, CI approval, deployment, scientific validity, or clinical authority",
                "waiver publicity is represented in the returned evidence record but no external publication is performed",
            ],
        }))
    }

    pub(super) fn bioeval_design_audit(&self, arguments: &Value) -> Result<Value, String> {
        const SCHEMA: &str = "bioprism-mcp/bioeval-design-audit/0.1";
        const MAX_FACTORS: usize = 256;
        const MAX_ARMS: usize = 4_096;
        const MAX_ID_BYTES: usize = 256;

        let refusal = |stage: &str, detail: String| {
            json!({
                "ok": false,
                "schema": SCHEMA,
                "workflow": "bioeval_design_audit",
                "stage": stage,
                "refusal": detail,
                "fail_closed": true,
                "guarantees": [
                    "partial factor assignments never become baseline values",
                    "only pairs differing in one declared factor become component contrasts",
                    "interaction estimability requires every observed two-by-two cell",
                    "multi-factor baseline differences remain unattributable",
                ],
                "limitations": [
                    "the route audits caller-supplied factorial arms and does not run an experiment or estimate an effect",
                    "paired seeds, cost/latency deltas, hidden-confounder detection, and sign-reversal search are outside this kernel",
                    "a controlled contrast carries a causal claim label from the attribution kernel but does not establish biological causality",
                ],
            })
        };
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot measure design audit input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("design audit input exceeds the 20000000-byte safety bound".into());
        }
        let cell_id = arguments
            .get("cell_id")
            .and_then(Value::as_str)
            .ok_or("cell_id is required and must be a string")?;
        let baseline = arguments
            .get("baseline")
            .and_then(Value::as_str)
            .ok_or("baseline is required and must name an arm")?;
        if cell_id.trim().is_empty()
            || cell_id.len() > MAX_ID_BYTES
            || baseline.trim().is_empty()
            || baseline.len() > MAX_ID_BYTES
        {
            return Ok(refusal(
                "design_validation",
                format!("cell_id and baseline must contain 1 to {MAX_ID_BYTES} bytes"),
            ));
        }
        let raw_factors = arguments
            .get("factors")
            .and_then(Value::as_array)
            .ok_or("factors is required and must be an array of factor names")?;
        if raw_factors.is_empty() || raw_factors.len() > MAX_FACTORS {
            return Err("factors must contain 1 to 256 names".into());
        }
        let mut factors = Vec::with_capacity(raw_factors.len());
        let mut factor_ids = BTreeSet::new();
        for (index, raw) in raw_factors.iter().enumerate() {
            let factor = raw
                .as_str()
                .ok_or_else(|| format!("factors[{index}] must be a string"))?;
            if factor.trim().is_empty() || factor.len() > MAX_ID_BYTES {
                return Ok(refusal(
                    "factor_validation",
                    format!("factors[{index}] must contain 1 to {MAX_ID_BYTES} bytes"),
                ));
            }
            if !factor_ids.insert(factor.to_string()) {
                return Ok(refusal(
                    "factor_validation",
                    format!("factor {:?} appears more than once", factor),
                ));
            }
            factors.push(factor.to_string());
        }
        let raw_arms = arguments
            .get("arms")
            .and_then(Value::as_array)
            .ok_or("arms is required and must be an array of serialized Arm values")?;
        if raw_arms.len() > MAX_ARMS {
            return Err("arms are bounded at 4096 rows".into());
        }
        let controlled = match arguments.get("controlled") {
            Some(value) => value
                .as_bool()
                .ok_or("controlled must be a boolean when supplied")?,
            None => false,
        };
        let max_items = arguments
            .get("max_items")
            .and_then(Value::as_u64)
            .unwrap_or(100);
        if !(1..=1_000).contains(&max_items) {
            return Err("max_items must be between 1 and 1000".into());
        }
        let max_items = max_items as usize;
        let require_contrasts = arguments
            .get("require_contrasts")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let require_complete_interactions = arguments
            .get("require_complete_interactions")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let require_attribution = arguments
            .get("require_attribution")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let mut design = BioevalFactorialDesign::declare(
            cell_id.to_string(),
            factors.clone(),
            baseline.to_string(),
        );
        for (index, raw) in raw_arms.iter().enumerate() {
            let arm: BioevalDesignArm = match serde_json::from_value(raw.clone()) {
                Ok(arm) => arm,
                Err(error) => {
                    return Ok(refusal(
                        "arm_deserialization",
                        format!("arms[{index}] is not a valid Arm: {error}"),
                    ));
                }
            };
            if arm.id.trim().is_empty() || arm.id.len() > MAX_ID_BYTES {
                return Ok(refusal(
                    "arm_validation",
                    format!("arms[{index}].id must contain 1 to {MAX_ID_BYTES} bytes"),
                ));
            }
            for (factor, level) in &arm.levels {
                if factor.len() > MAX_ID_BYTES
                    || level.trim().is_empty()
                    || level.len() > MAX_ID_BYTES
                {
                    return Ok(refusal(
                        "arm_validation",
                        format!(
                            "arms[{index}] has a factor or level outside the {MAX_ID_BYTES}-byte bound"
                        ),
                    ));
                }
            }
            if let Err(error) = design.add(arm) {
                return Ok(refusal("arm_validation", error.to_string()));
            }
        }
        if let Err(error) = design.validate() {
            return Ok(refusal("design_validation", error.to_string()));
        }

        let contrasts = design.single_factor_contrasts();
        if require_contrasts && contrasts.is_empty() {
            return Ok(refusal(
                "contrast_coverage",
                "require_contrasts was true but no pair differs in exactly one factor".into(),
            ));
        }
        let mut interaction_rows = Vec::new();
        let mut missing_interactions = BTreeSet::new();
        let factor_vec = design.factors().iter().cloned().collect::<Vec<_>>();
        for (index, factor_a) in factor_vec.iter().enumerate() {
            for factor_b in factor_vec.iter().skip(index + 1) {
                let missing = design.missing_for_interaction(factor_a, factor_b);
                if !missing.is_empty() {
                    missing_interactions.insert(format!("{factor_a}::{factor_b}"));
                }
                interaction_rows.push(json!({
                    "factors": [factor_a, factor_b],
                    "estimable": missing.is_empty(),
                    "missing_cells": missing.iter().map(|(a, b)| json!({ "level_a": a, "level_b": b })).collect::<Vec<_>>(),
                }));
            }
        }
        if require_complete_interactions && !missing_interactions.is_empty() {
            return Ok(refusal(
                "interaction_coverage",
                format!(
                    "require_complete_interactions was true but {} factor pair(s) are missing cells",
                    missing_interactions.len()
                ),
            ));
        }

        let forks = match design.contrast_forks(controlled) {
            Ok(forks) => forks,
            Err(error) => return Ok(refusal("contrast_generation", error.to_string())),
        };
        let attribution_rows = forks
            .iter()
            .take(max_items)
            .map(|fork| {
                let attribution = bioeval_design_attribute(fork);
                json!({
                    "fork": fork,
                    "attribution": attribution,
                    "explanation": attribution.explain(),
                    "refused": attribution.is_refused(),
                    "causal": attribution.is_causal(),
                })
            })
            .collect::<Vec<_>>();
        let attribution_total = forks.len();
        let refused_attributions = forks
            .iter()
            .filter(|fork| bioeval_design_attribute(fork).is_refused())
            .count();
        if require_attribution && refused_attributions > 0 {
            return Ok(refusal(
                "attribution_policy",
                format!(
                    "require_attribution was true but {refused_attributions} generated contrast(s) were refused"
                ),
            ));
        }
        let unattributable = design
            .unattributable()
            .into_iter()
            .map(str::to_string)
            .collect::<BTreeSet<_>>();
        let arm_rows = design
            .arms()
            .iter()
            .take(max_items)
            .map(|arm| {
                json!({
                    "id": arm.id,
                    "levels": arm.levels,
                    "conclusion": arm.conclusion,
                    "tier": arm.tier,
                    "baseline": arm.id == baseline,
                    "unattributable_from_baseline": unattributable.contains(&arm.id),
                })
            })
            .collect::<Vec<_>>();
        let bounded_ids = |ids: &BTreeSet<String>| {
            json!({
                "ids": ids.iter().take(max_items).cloned().collect::<Vec<_>>(),
                "total": ids.len(),
                "omitted": ids.len().saturating_sub(max_items),
            })
        };
        Ok(json!({
            "ok": true,
            "schema": SCHEMA,
            "workflow": "bioeval_design_audit",
            "design": {
                "cell_id": cell_id,
                "factors": factor_vec,
                "baseline": baseline,
                "arm_count": design.arms().len(),
                "contrast_count": contrasts.len(),
                "unattributable_arm_count": unattributable.len(),
                "controlled": controlled,
                "valid": true,
            },
            "arms": {
                "rows": arm_rows,
                "returned": design.arms().len().min(max_items),
                "total": design.arms().len(),
                "omitted": design.arms().len().saturating_sub(max_items),
            },
            "contrasts": {
                "rows": contrasts.iter().take(max_items).collect::<Vec<_>>(),
                "returned": contrasts.len().min(max_items),
                "total": contrasts.len(),
                "omitted": contrasts.len().saturating_sub(max_items),
            },
            "interactions": {
                "rows": interaction_rows.iter().take(max_items).cloned().collect::<Vec<_>>(),
                "returned": interaction_rows.len().min(max_items),
                "total": interaction_rows.len(),
                "omitted": interaction_rows.len().saturating_sub(max_items),
                "estimable_count": interaction_rows.iter().filter(|row| row["estimable"] == json!(true)).count(),
                "missing_count": missing_interactions.len(),
            },
            "attributions": {
                "rows": attribution_rows,
                "returned": attribution_total.min(max_items),
                "total": attribution_total,
                "omitted": attribution_total.saturating_sub(max_items),
                "refused_count": refused_attributions,
                "causal_count": forks.iter().filter(|fork| bioeval_design_attribute(fork).is_causal()).count(),
            },
            "findings": {
                "unattributable_arms": bounded_ids(&unattributable),
                "missing_interactions": bounded_ids(&missing_interactions),
                "no_single_factor_contrasts": contrasts.is_empty(),
                "attribution_refusal_count": refused_attributions,
            },
            "guarantees": [
                "incomplete arms and undeclared factors fail before contrast generation",
                "baseline selection is explicit and never inferred from observed conclusions",
                "contrasts differ in exactly one factor and identify the held-fixed factors",
                "interaction rows name missing two-by-two cells rather than reporting an unestimable effect",
                "unattributable multi-factor arms remain visible and are not folded into component counts",
                "bounded arm, contrast, interaction, attribution, and finding projections retain total and omitted counts",
            ],
            "limitations": [
                "the route does not execute arms, estimate effect sizes, randomize, pair seeds, or measure cost/latency",
                "controlled is a caller-supplied design declaration; a causal label is not independent causal verification",
                "interaction rows establish cell coverage only and do not calculate an interaction effect",
                "a valid design is not biological, clinical, or deployment validity",
            ],
        }))
    }
}
