//! Minimum-evidence-cut planning for contradictory preclinical glioma surveillance.
//!
//! Triangulation reports whether a claim is currently supportable. This feature answers the next
//! operational question: which smallest set of evidence records should be audited first to cover
//! the active disagreement edges, and which claims still require an independent replication? The
//! result is a deterministic weighted vertex-cover approximation over signed evidence pairs. It
//! plans review and replication; it never edits evidence or promotes a claim.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F08";
pub const OUTPUT_SCHEMA: &str = "GliomaEvidenceContradictionCut1@1";
pub const MAX_EVIDENCE: usize = 16_384;
pub const MAX_CLAIMS: usize = 4_096;
pub const MAX_AUDITS: usize = 4_096;

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

fn select_cut(
    conflicts: &[ContradictionConflict],
    evidence: &BTreeMap<String, &ContradictionEvidence>,
    request: &ContradictionCutRequest,
) -> (Vec<ContradictionAuditSelection>, BTreeSet<String>, u64) {
    let mut edges = BTreeSet::<(String, String)>::new();
    for conflict in conflicts {
        for support in &conflict.support_order {
            for contradict in &conflict.contradict_order {
                edges.insert((support.clone(), contradict.clone()));
            }
        }
    }
    let mut remaining = edges;
    let mut selected = Vec::new();
    let mut selected_ids = BTreeSet::new();
    let mut budget = request.budget_units;
    while !remaining.is_empty() && selected.len() < request.max_audits {
        let mut best: Option<(String, usize, u32, u16)> = None;
        for (id, item) in evidence {
            if selected_ids.contains(id) || u64::from(item.audit_cost_units) > budget {
                continue;
            }
            let coverage = remaining
                .iter()
                .filter(|(left, right)| left == id || right == id)
                .count();
            if coverage == 0 {
                continue;
            }
            let priority = ((coverage as u64)
                .saturating_mul(u64::from(item.confidence_milli))
                .saturating_mul(1_000)
                .checked_div(u64::from(item.audit_cost_units).max(1))
                .unwrap_or(0))
            .min(1_000) as u16;
            let candidate = (id.clone(), coverage, item.audit_cost_units, priority);
            if best
                .as_ref()
                .map(|current| {
                    candidate.3 > current.3
                        || (candidate.3 == current.3 && candidate.1 > current.1)
                        || (candidate.3 == current.3
                            && candidate.1 == current.1
                            && candidate.0 < current.0)
                })
                .unwrap_or(true)
            {
                best = Some(candidate);
            }
        }
        let Some((id, coverage, cost, priority)) = best else {
            break;
        };
        let covered_claims = conflicts
            .iter()
            .filter(|conflict| {
                conflict.support_order.iter().any(|value| value == &id)
                    || conflict.contradict_order.iter().any(|value| value == &id)
            })
            .map(|conflict| conflict.claim_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        remaining.retain(|(left, right)| left != &id && right != &id);
        selected_ids.insert(id.clone());
        budget = budget.saturating_sub(u64::from(cost));
        selected.push(ContradictionAuditSelection {
            evidence_id: id,
            covered_claim_order: covered_claims,
            covered_edge_count: coverage,
            audit_cost_units: cost,
            priority_milli: priority,
            rationale: "audit this evidence record to cover the highest-weight unresolved disagreement edge".into(),
        });
    }
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
            .chain(&conflict.contradict_order)
            .any(|id| audited.contains(id));
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
        audit_order: selections
            .iter()
            .map(|selection| selection.evidence_id.clone())
            .collect(),
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
