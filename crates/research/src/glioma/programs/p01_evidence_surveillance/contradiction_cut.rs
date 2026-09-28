//! Beam-selected minimum-evidence-cut planning for contradictory preclinical glioma surveillance.
//!
//! Triangulation reports whether a claim is currently supportable. This feature answers the next
//! operational question: which smallest set of evidence records should be audited first to cover
//! the active disagreement edges, and which claims still require an independent replication? The
//! result is a deterministic weighted vertex-cover portfolio over signed evidence pairs. It plans
//! review and replication; it never edits evidence or promotes a claim.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F08";
pub const OUTPUT_SCHEMA: &str = "GliomaEvidenceContradictionCut1@2";
pub const MAX_EVIDENCE: usize = 16_384;
pub const MAX_CLAIMS: usize = 4_096;
pub const MAX_AUDITS: usize = 4_096;
const CUT_BEAM_WIDTH: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidencePolarity {
    Support,
    Contradict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContradictionEvidence {
    pub evidence_id: String,
    pub claim_id: String,
    pub polarity: EvidencePolarity,
    pub source_family: String,
    pub independence_group: String,
    pub confidence_milli: u16,
    pub audit_cost_units: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContradictionCutRequest {
    pub objective: String,
    pub min_confidence_milli: u16,
    pub max_audits: usize,
    pub budget_units: u64,
    pub require_independent_replication: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContradictionConflict {
    pub claim_id: String,
    pub support_order: Vec<String>,
    pub contradict_order: Vec<String>,
    pub source_family_order: Vec<String>,
    pub independent_group_count: usize,
    pub severity_milli: u16,
    pub covered_by_audit: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContradictionAuditSelection {
    pub evidence_id: String,
    pub covered_claim_order: Vec<String>,
    pub covered_edge_count: usize,
    pub audit_cost_units: u32,
    pub priority_milli: u16,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContradictionCutDisposition {
    NoContradiction,
    Covered,
    Partial,
    BudgetBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContradictionCut {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub claim_order: Vec<String>,
    pub conflicts: Vec<ContradictionConflict>,
    pub audit_order: Vec<String>,
    pub selections: Vec<ContradictionAuditSelection>,
    pub unresolved_claim_order: Vec<String>,
    pub next_action_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ContradictionCutDisposition,
    pub budget_remaining_units: u64,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContradictionCutError {
    #[error("contradiction-cut request is invalid: {0}")]
    InvalidRequest(String),
    #[error("contradiction-cut evidence is invalid: {0}")]
    InvalidEvidence(String),
    #[error("contradiction-cut output is invalid: {0}")]
    InvalidOutput(String),
    #[error("contradiction-cut digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &ContradictionCut) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "claim_order": output.claim_order,
        "conflicts": output.conflicts,
        "audit_order": output.audit_order,
        "selections": output.selections,
        "unresolved_claim_order": output.unresolved_claim_order,
        "next_action_order": output.next_action_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "budget_remaining_units": output.budget_remaining_units,
    })
}

impl ContradictionCut {
    pub fn validate(&self) -> Result<(), ContradictionCutError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.claim_order)
            || !canonical(&self.audit_order)
            || !canonical(&self.unresolved_claim_order)
            || !canonical(&self.next_action_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self
                .conflicts
                .windows(2)
                .any(|pair| pair[0].claim_id >= pair[1].claim_id)
            || self.selections.windows(2).any(|pair| {
                pair[0].priority_milli < pair[1].priority_milli
                    || (pair[0].priority_milli == pair[1].priority_milli
                        && pair[0].evidence_id > pair[1].evidence_id)
            })
            || self.selections.iter().any(|selection| {
                selection.evidence_id.trim().is_empty()
                    || selection.covered_edge_count == 0
                    || selection.audit_cost_units == 0
                    || selection.priority_milli > 1_000
                    || selection.rationale.trim().is_empty()
                    || !canonical(&selection.covered_claim_order)
            })
        {
            return Err(ContradictionCutError::InvalidOutput(
                "identity, ordering, conflict, or selection bounds are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ContradictionCutError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ContradictionCutError::InvalidOutput(
                "digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &ContradictionCutRequest) -> Result<(), ContradictionCutError> {
    if request.objective.trim().is_empty()
        || request.min_confidence_milli > 1_000
        || request.max_audits == 0
        || request.max_audits > MAX_AUDITS
        || request.budget_units == 0
    {
        return Err(ContradictionCutError::InvalidRequest(
            "objective, confidence bound, bounded audits, and positive budget are required".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CutState {
    selected: Vec<String>,
    covered_edges: BTreeSet<(String, String)>,
    covered_edge_counts: BTreeMap<String, usize>,
    touched_claims: BTreeSet<String>,
    resolved_claims: BTreeSet<String>,
    independence_groups: BTreeSet<String>,
    source_families: BTreeSet<String>,
    spent_units: u64,
}

fn cut_state_score(
    state: &CutState,
    evidence: &BTreeMap<String, &ContradictionEvidence>,
    claim_severity: &BTreeMap<String, u16>,
    isolated_claim: bool,
) -> u128 {
    let confidence = state
        .selected
        .iter()
        .filter_map(|id| evidence.get(id))
        .map(|item| u128::from(item.confidence_milli))
        .sum::<u128>();
    // For one isolated disagreement, prefer auditing the contradiction anchor so the next action
    // directly interrogates the surprising edge. Once a portfolio spans multiple claims, retain
    // the low-cost support-side tie-break that favors independent positive anchors and preserves
    // the multi-claim diversity behavior of the cut beam.
    let (support_preference, contradiction_preference) =
        if isolated_claim && state.touched_claims.len() <= 1 {
            (
                0,
                state
                    .selected
                    .iter()
                    .filter_map(|id| evidence.get(id))
                    .filter(|item| item.polarity == EvidencePolarity::Contradict)
                    .count() as u128,
            )
        } else {
            (
                state
                    .selected
                    .iter()
                    .filter_map(|id| evidence.get(id))
                    .filter(|item| item.polarity == EvidencePolarity::Support)
                    .count() as u128,
                0,
            )
        };
    // A claim is resolved only when every signed support/contradiction edge is covered. Partial
    // touches remain useful for search, but cannot masquerade as a closed contradiction. Distinct
    // resolved claims are the scientific objective; edge count, severity, diversity, and cost are
    // deterministic refinements.
    let resolved = (state.resolved_claims.len() as u128).saturating_mul(1_000_000_000_000);
    let resolved_severity = state
        .resolved_claims
        .iter()
        .filter_map(|claim| claim_severity.get(claim))
        .map(|severity| u128::from(*severity))
        .sum::<u128>()
        .saturating_mul(100_000_000);
    let partial = (state.touched_claims.len() as u128).saturating_mul(100_000);
    let edge_coverage = (state.covered_edges.len() as u128).saturating_mul(1_000_000);
    let independence = (state.independence_groups.len() as u128).saturating_mul(10_000_000_000);
    let source_diversity = (state.source_families.len() as u128).saturating_mul(1_000_000_000);
    let confidence = confidence.saturating_mul(1_000_000);
    resolved
        .saturating_add(resolved_severity)
        .saturating_add(partial)
        .saturating_add(edge_coverage)
        .saturating_add(independence)
        .saturating_add(source_diversity)
        .saturating_add(support_preference.saturating_mul(10_000_000))
        .saturating_add(contradiction_preference.saturating_mul(10_000_000))
        .saturating_add(confidence)
        .saturating_sub(u128::from(state.spent_units).saturating_mul(1_000))
}

/// Search evidence-record portfolios rather than committing to the first high-degree vertex.
/// The objective gives contradiction-edge coverage first priority, then independent source-group
/// coverage and confidence, with a small cost penalty. Every candidate is still bounded by the
/// declared audit count and budget, and zero-new-edge records are never selected.
fn select_cut(
    conflicts: &[ContradictionConflict],
    evidence: &BTreeMap<String, &ContradictionEvidence>,
    request: &ContradictionCutRequest,
) -> (Vec<ContradictionAuditSelection>, BTreeSet<String>, u64) {
    let mut edges = BTreeSet::<(String, String)>::new();
    let mut edge_claim = BTreeMap::<(String, String), String>::new();
    let mut claim_edge_counts = BTreeMap::<String, usize>::new();
    let mut claim_severity = BTreeMap::<String, u16>::new();
    for conflict in conflicts {
        for support in &conflict.support_order {
            for contradict in &conflict.contradict_order {
                let edge = (support.clone(), contradict.clone());
                if edges.insert(edge.clone()) {
                    edge_claim.insert(edge, conflict.claim_id.clone());
                    *claim_edge_counts
                        .entry(conflict.claim_id.clone())
                        .or_default() += 1;
                }
            }
        }
        claim_severity.insert(conflict.claim_id.clone(), conflict.severity_milli);
    }
    let candidate_ids = evidence.keys().cloned().collect::<Vec<_>>();
    let mut states = vec![CutState {
        selected: Vec::new(),
        covered_edges: BTreeSet::new(),
        covered_edge_counts: BTreeMap::new(),
        touched_claims: BTreeSet::new(),
        resolved_claims: BTreeSet::new(),
        independence_groups: BTreeSet::new(),
        source_families: BTreeSet::new(),
        spent_units: 0,
    }];
    for id in candidate_ids {
        let item = evidence[&id];
        let mut next = states.clone();
        for state in &states {
            if state.selected.len() >= request.max_audits
                || state
                    .spent_units
                    .saturating_add(u64::from(item.audit_cost_units))
                    > request.budget_units
            {
                continue;
            }
            let new_edges = edges
                .iter()
                .filter(|edge| {
                    (edge.0 == id || edge.1 == id) && !state.covered_edges.contains(*edge)
                })
                .cloned()
                .collect::<BTreeSet<_>>();
            if new_edges.is_empty() {
                continue;
            }
            let mut selected = state.selected.clone();
            selected.push(id.clone());
            let mut covered_edges = state.covered_edges.clone();
            covered_edges.extend(new_edges.iter().cloned());
            let mut covered_edge_counts = state.covered_edge_counts.clone();
            for edge in &new_edges {
                if let Some(claim_id) = edge_claim.get(edge) {
                    *covered_edge_counts.entry(claim_id.clone()).or_default() += 1;
                }
            }
            let mut touched_claims = state.touched_claims.clone();
            let mut resolved_claims = state.resolved_claims.clone();
            for edge in &new_edges {
                if let Some(claim_id) = edge_claim.get(edge) {
                    touched_claims.insert(claim_id.clone());
                    if covered_edge_counts.get(claim_id).copied().unwrap_or(0)
                        >= claim_edge_counts
                            .get(claim_id)
                            .copied()
                            .unwrap_or(usize::MAX)
                    {
                        resolved_claims.insert(claim_id.clone());
                    }
                }
            }
            let mut independence_groups = state.independence_groups.clone();
            independence_groups.insert(item.independence_group.clone());
            let mut source_families = state.source_families.clone();
            source_families.insert(item.source_family.clone());
            next.push(CutState {
                selected,
                covered_edges,
                covered_edge_counts,
                touched_claims,
                resolved_claims,
                independence_groups,
                source_families,
                spent_units: state
                    .spent_units
                    .saturating_add(u64::from(item.audit_cost_units)),
            });
        }
        next.sort_by(|left, right| {
            cut_state_score(right, evidence, &claim_severity, conflicts.len() == 1)
                .cmp(&cut_state_score(
                    left,
                    evidence,
                    &claim_severity,
                    conflicts.len() == 1,
                ))
                .then_with(|| left.spent_units.cmp(&right.spent_units))
                .then_with(|| left.selected.cmp(&right.selected))
        });
        next.dedup_by(|left, right| left.selected == right.selected);
        next.truncate(CUT_BEAM_WIDTH);
        states = next;
    }
    let chosen = states
        .into_iter()
        .max_by(|left, right| {
            cut_state_score(left, evidence, &claim_severity, conflicts.len() == 1)
                .cmp(&cut_state_score(
                    right,
                    evidence,
                    &claim_severity,
                    conflicts.len() == 1,
                ))
                .then_with(|| right.spent_units.cmp(&left.spent_units))
                .then_with(|| right.selected.cmp(&left.selected))
        })
        .expect("contradiction-cut beam always retains an empty state");

    let mut remaining = edges;
    let mut budget = request.budget_units;
    let mut selected = Vec::new();
    for id in chosen.selected {
        let item = evidence[&id];
        let covered_edges = remaining
            .iter()
            .filter(|(left, right)| left == &id || right == &id)
            .cloned()
            .collect::<BTreeSet<_>>();
        if covered_edges.is_empty() {
            continue;
        }
        let coverage = covered_edges.len();
        let priority = ((coverage as u64)
            .saturating_mul(u64::from(item.confidence_milli))
            .saturating_mul(1_000)
            .checked_div(u64::from(item.audit_cost_units).max(1))
            .unwrap_or(0))
        .min(1_000) as u16;
        let covered_claims = conflicts
            .iter()
            .filter(|conflict| {
                conflict.support_order.iter().any(|value| value == &id)
                    || conflict.contradict_order.iter().any(|value| value == &id)
            })
            .map(|conflict| conflict.claim_id.clone())
            .collect::<BTreeSet<_>>();
        let covered_claim_order = covered_claims.iter().cloned().collect::<Vec<_>>();
        // Remove only the signed edges incident to this audited record. A partial vertex cover
        // must remain visible so the autonomous loop can schedule the complementary support or
        // contradiction audit instead of falsely closing a multi-record conflict.
        remaining.retain(|edge| edge.0 != id && edge.1 != id);
        budget = budget.saturating_sub(u64::from(item.audit_cost_units));
        selected.push(ContradictionAuditSelection {
            evidence_id: id,
            covered_claim_order,
            covered_edge_count: coverage,
            audit_cost_units: item.audit_cost_units,
            priority_milli: priority,
            rationale:
                "audit this evidence record to cover a globally selected disagreement portfolio"
                    .into(),
        });
    }
    selected.sort_by(|left, right| {
        right
            .priority_milli
            .cmp(&left.priority_milli)
            .then_with(|| left.evidence_id.cmp(&right.evidence_id))
    });
    (
        selected,
        remaining
            .into_iter()
            .flat_map(|(left, right)| [left, right])
            .collect(),
        budget,
    )
}

/// Compute a deterministic evidence cut over contradictory signed records and return the next
/// audit/replication actions for the P01 surveillance loop.
pub fn plan_glioma_evidence_contradiction_cut(
    request: &ContradictionCutRequest,
    evidence: &[ContradictionEvidence],
) -> Result<ContradictionCut, ContradictionCutError> {
    validate_request(request)?;
    if evidence.is_empty() || evidence.len() > MAX_EVIDENCE {
        return Err(ContradictionCutError::InvalidEvidence(
            "a bounded non-empty evidence set is required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    let mut by_claim = BTreeMap::<String, Vec<&ContradictionEvidence>>::new();
    let mut by_id = BTreeMap::new();
    for item in evidence {
        if item.evidence_id.trim().is_empty()
            || item.claim_id.trim().is_empty()
            || item.source_family.trim().is_empty()
            || item.independence_group.trim().is_empty()
            || item.confidence_milli < request.min_confidence_milli
            || item.audit_cost_units == 0
            || !ids.insert(item.evidence_id.clone())
        {
            return Err(ContradictionCutError::InvalidEvidence(
                "evidence identity, confidence, cost, and uniqueness are required".into(),
            ));
        }
        by_id.insert(item.evidence_id.clone(), item);
        by_claim
            .entry(item.claim_id.clone())
            .or_default()
            .push(item);
    }
    if by_claim.len() > MAX_CLAIMS {
        return Err(ContradictionCutError::InvalidEvidence(
            "claim count exceeds bounded contradiction capacity".into(),
        ));
    }
    let mut conflicts = Vec::new();
    for (claim_id, items) in &by_claim {
        let mut support = items
            .iter()
            .filter(|item| item.polarity == EvidencePolarity::Support)
            .map(|item| item.evidence_id.clone())
            .collect::<Vec<_>>();
        let mut contradict = items
            .iter()
            .filter(|item| item.polarity == EvidencePolarity::Contradict)
            .map(|item| item.evidence_id.clone())
            .collect::<Vec<_>>();
        support.sort();
        contradict.sort();
        if support.is_empty() || contradict.is_empty() {
            continue;
        }
        let source_family_order = items
            .iter()
            .map(|item| item.source_family.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let independent_groups = items
            .iter()
            .map(|item| item.independence_group.clone())
            .collect::<BTreeSet<_>>();
        let severity = items
            .iter()
            .map(|item| u64::from(item.confidence_milli))
            .min()
            .unwrap_or(0) as u16;
        conflicts.push(ContradictionConflict {
            claim_id: claim_id.clone(),
            support_order: support,
            contradict_order: contradict,
            source_family_order,
            independent_group_count: independent_groups.len(),
            severity_milli: severity,
            covered_by_audit: false,
        });
    }
    conflicts.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
    let (selections, uncovered_ids, budget_remaining_units) =
        select_cut(&conflicts, &by_id, request);
    let audited = selections
        .iter()
        .map(|selection| selection.evidence_id.clone())
        .collect::<BTreeSet<_>>();
    for conflict in &mut conflicts {
        conflict.covered_by_audit = conflict
            .support_order
            .iter()
            .flat_map(|support| {
                conflict
                    .contradict_order
                    .iter()
                    .map(move |contradict| (support, contradict))
            })
            .all(|(support, contradict)| audited.contains(support) || audited.contains(contradict));
    }
    let claim_order = by_claim.keys().cloned().collect::<Vec<_>>();
    let unresolved_claim_order = conflicts
        .iter()
        .filter(|conflict| !conflict.covered_by_audit)
        .map(|conflict| conflict.claim_id.clone())
        .collect::<Vec<_>>();
    let mut next_action_order = selections
        .iter()
        .map(|selection| format!("audit:{}", selection.evidence_id))
        .collect::<Vec<_>>();
    if request.require_independent_replication {
        next_action_order.extend(
            unresolved_claim_order
                .iter()
                .map(|claim| format!("independent-replication:{claim}")),
        );
    }
    next_action_order.sort();
    next_action_order.dedup();
    let mut negative_evidence = conflicts
        .iter()
        .map(|conflict| format!("claim:{}:contradictory-evidence", conflict.claim_id))
        .collect::<Vec<_>>();
    let mut uncertainty = Vec::new();
    for conflict in &conflicts {
        if conflict.independent_group_count < 2 {
            uncertainty.push(format!(
                "claim:{}:contradiction-lacks-independent-source-group",
                conflict.claim_id
            ));
        }
    }
    if !uncovered_ids.is_empty() {
        uncertainty.push("audit-budget-or-capacity-left-disagreement-edges-uncovered".into());
    }
    negative_evidence.sort();
    uncertainty.sort();
    // `selections` is execution-priority ordered; keep the separately exposed ID order canonical
    // so cross-language consumers can compare the catalog without depending on scheduling ties.
    let mut audit_order = selections
        .iter()
        .map(|selection| selection.evidence_id.clone())
        .collect::<Vec<_>>();
    audit_order.sort();
    let disposition = if conflicts.is_empty() {
        ContradictionCutDisposition::NoContradiction
    } else if unresolved_claim_order.is_empty() {
        ContradictionCutDisposition::Covered
    } else if selections.is_empty() || budget_remaining_units == 0 {
        ContradictionCutDisposition::BudgetBlocked
    } else {
        ContradictionCutDisposition::Partial
    };
    let mut output = ContradictionCut {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        claim_order,
        conflicts,
        audit_order,
        selections,
        unresolved_claim_order,
        next_action_order,
        negative_evidence,
        uncertainty,
        disposition,
        budget_remaining_units,
        digest: ContentHash::of_bytes(b"unsealed-contradiction-cut"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ContradictionCutError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(budget_units: u64) -> ContradictionCutRequest {
        ContradictionCutRequest {
            objective: "resolve invasion evidence conflict".into(),
            min_confidence_milli: 100,
            max_audits: 4,
            budget_units,
            require_independent_replication: true,
        }
    }

    fn evidence(
        id: &str,
        polarity: EvidencePolarity,
        claim: &str,
        cost: u32,
    ) -> ContradictionEvidence {
        ContradictionEvidence {
            evidence_id: id.into(),
            claim_id: claim.into(),
            polarity,
            source_family: format!("source-{id}"),
            independence_group: format!("group-{id}"),
            confidence_milli: 800,
            audit_cost_units: cost,
        }
    }

    #[test]
    fn greedy_cut_covers_a_contradiction_and_emits_audit_action() {
        let output = plan_glioma_evidence_contradiction_cut(
            &request(3),
            &[
                evidence("support", EvidencePolarity::Support, "claim-a", 1),
                evidence("contradict", EvidencePolarity::Contradict, "claim-a", 1),
            ],
        )
        .unwrap();
        assert_eq!(output.disposition, ContradictionCutDisposition::Covered);
        assert_eq!(output.conflicts[0].covered_by_audit, true);
        assert!(output
            .next_action_order
            .iter()
            .any(|item| item == "audit:contradict"));
        output.validate().unwrap();
    }

    #[test]
    fn cut_beam_prefers_exact_resolution_over_partial_independent_touches() {
        let values = vec![
            evidence("hub", EvidencePolarity::Support, "claim-a", 2),
            evidence("cheap-a", EvidencePolarity::Support, "claim-a", 1),
            evidence("cheap-b", EvidencePolarity::Support, "claim-b", 1),
            evidence("contradict-a", EvidencePolarity::Contradict, "claim-a", 1),
            evidence("contradict-b", EvidencePolarity::Contradict, "claim-b", 1),
        ];
        let by_id = values
            .iter()
            .map(|item| (item.evidence_id.clone(), item))
            .collect::<BTreeMap<_, _>>();
        let conflicts = vec![
            ContradictionConflict {
                claim_id: "claim-a".into(),
                support_order: vec!["cheap-a".into(), "hub".into()],
                contradict_order: vec!["contradict-a".into()],
                source_family_order: vec!["source-a".into()],
                independent_group_count: 2,
                severity_milli: 800,
                covered_by_audit: false,
            },
            ContradictionConflict {
                claim_id: "claim-b".into(),
                support_order: vec!["cheap-b".into(), "hub".into()],
                contradict_order: vec!["contradict-b".into()],
                source_family_order: vec!["source-b".into()],
                independent_group_count: 2,
                severity_milli: 800,
                covered_by_audit: false,
            },
        ];
        let (selected, uncovered, remaining_budget) = select_cut(
            &conflicts,
            &by_id,
            &ContradictionCutRequest {
                objective: "resolve two independent conflicts".into(),
                min_confidence_milli: 100,
                max_audits: 2,
                budget_units: 2,
                require_independent_replication: true,
            },
        );
        assert_eq!(
            selected
                .iter()
                .map(|selection| selection.evidence_id.as_str())
                .collect::<Vec<_>>(),
            vec!["contradict-a", "contradict-b"]
        );
        assert!(uncovered.is_empty());
        assert_eq!(remaining_budget, 0);
    }

    #[test]
    fn partial_vertex_cover_keeps_multi_record_conflict_unresolved() {
        let output = plan_glioma_evidence_contradiction_cut(
            &ContradictionCutRequest {
                objective: "resolve multi-record conflict".into(),
                min_confidence_milli: 100,
                max_audits: 1,
                budget_units: 1,
                require_independent_replication: true,
            },
            &[
                evidence("support-a", EvidencePolarity::Support, "claim-a", 1),
                evidence("support-b", EvidencePolarity::Support, "claim-a", 1),
                evidence("contradict-a", EvidencePolarity::Contradict, "claim-a", 1),
                evidence("contradict-b", EvidencePolarity::Contradict, "claim-a", 1),
            ],
        )
        .unwrap();
        assert_eq!(
            output.disposition,
            ContradictionCutDisposition::BudgetBlocked
        );
        assert_eq!(output.unresolved_claim_order, vec!["claim-a"]);
        assert!(!output.conflicts[0].covered_by_audit);
        assert!(output
            .next_action_order
            .iter()
            .any(|action| action == "independent-replication:claim-a"));
    }

    #[test]
    fn budget_block_preserves_unresolved_claim() {
        let mut request = request(1);
        request.max_audits = 1;
        let output = plan_glioma_evidence_contradiction_cut(
            &request,
            &[
                evidence("support", EvidencePolarity::Support, "claim-a", 2),
                evidence("contradict", EvidencePolarity::Contradict, "claim-a", 2),
            ],
        )
        .unwrap();
        assert_eq!(
            output.disposition,
            ContradictionCutDisposition::BudgetBlocked
        );
        assert_eq!(output.unresolved_claim_order, vec!["claim-a"]);
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item.contains("uncovered")));
    }

    #[test]
    fn input_order_does_not_change_cut_digest() {
        let request = request(3);
        let left = plan_glioma_evidence_contradiction_cut(
            &request,
            &[
                evidence("support", EvidencePolarity::Support, "claim-a", 1),
                evidence("contradict", EvidencePolarity::Contradict, "claim-a", 1),
            ],
        )
        .unwrap();
        let right = plan_glioma_evidence_contradiction_cut(
            &request,
            &[
                evidence("contradict", EvidencePolarity::Contradict, "claim-a", 1),
                evidence("support", EvidencePolarity::Support, "claim-a", 1),
            ],
        )
        .unwrap();
        assert_eq!(left.digest, right.digest);
    }
}
