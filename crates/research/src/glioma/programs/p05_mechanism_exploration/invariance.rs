//! Cross-model mechanistic invariance frontier for preclinical glioma research.
//!
//! A signature that separates mechanisms in one model system can disappear, reverse direction,
//! or become unmeasurable in another. This feature evaluates signed mechanism predictions across
//! declared preclinical contexts, combines directional consistency with pairwise separation, and
//! greedily selects a bounded panel of signatures that remains useful across contexts. It is a
//! transportability planning artifact: it never treats a prediction as an observation, moves raw
//! data, executes biology, or makes a clinical decision.

use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F03";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismInvariance1@1";
pub const MAX_CONTEXTS: usize = 256;
pub const MAX_MECHANISMS: usize = 256;
pub const MAX_SIGNATURES: usize = 4_096;
pub const SCORE_SCALE: u64 = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismInvarianceContext {
    pub context_id: String,
    pub label: String,
    pub model_system: GliomaModelSystem,
    pub weight_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvarianceMechanism {
    pub mechanism_id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismSignature {
    pub signature_id: String,
    pub label: String,
    pub value_milli_by_context_and_mechanism: BTreeMap<String, BTreeMap<String, i32>>,
    pub quality_milli: u16,
    pub cost_units: u32,
    pub risk_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismInvarianceRequest {
    pub objective: String,
    pub contexts: Vec<MechanismInvarianceContext>,
    pub mechanisms: Vec<InvarianceMechanism>,
    pub signatures: Vec<MechanismSignature>,
    pub min_contexts: usize,
    pub invariance_floor_milli: u64,
    pub separation_floor_milli: u64,
    pub min_signature_quality_milli: u16,
    pub risk_ceiling_milli: u16,
    pub budget_units: u64,
    pub max_selected_signatures: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismInvariancePair {
    pub pair_id: String,
    pub left_mechanism_id: String,
    pub right_mechanism_id: String,
    pub selected_signature_order: Vec<String>,
    pub separation_milli: u64,
    pub transport_stability_milli: u64,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismInvarianceSignatureScore {
    pub signature_id: String,
    pub label: String,
    pub context_order: Vec<String>,
    pub context_count: usize,
    pub direction_consistency_milli: u64,
    pub separation_milli: u64,
    pub transport_stability_milli: u64,
    pub invariance_score_milli: u64,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismInvarianceUtility {
    pub signature_id: String,
    pub marginal_value_milli: u64,
    pub covered_pair_order: Vec<String>,
    pub projected_cost_units: u64,
    pub action: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismInvarianceDisposition {
    Qualified,
    Partial,
    NoEligibleSignatures,
    BudgetBlocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismInvariance {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub context_order: Vec<String>,
    pub mechanism_order: Vec<String>,
    pub signature_order: Vec<String>,
    pub selected_signature_order: Vec<String>,
    pub quality_blocked_order: Vec<String>,
    pub risk_blocked_order: Vec<String>,
    pub context_blocked_order: Vec<String>,
    pub budget_blocked_order: Vec<String>,
    pub pairs: Vec<MechanismInvariancePair>,
    pub scores: Vec<MechanismInvarianceSignatureScore>,
    pub utilities: Vec<MechanismInvarianceUtility>,
    pub unresolved_pair_order: Vec<String>,
    pub budget_remaining_units: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: MechanismInvarianceDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismInvarianceError {
    #[error("mechanism invariance request is invalid: {0}")]
    InvalidRequest(String),
    #[error("mechanism invariance input is invalid: {0}")]
    InvalidInput(String),
    #[error("mechanism invariance output is invalid: {0}")]
    InvalidOutput(String),
    #[error("mechanism invariance digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn pair_id(left: &str, right: &str) -> String {
    format!("{left}__{right}")
}

fn weighted_majority_consistency(values: impl Iterator<Item = (i64, u16)>) -> u64 {
    let mut positive = 0_u64;
    let mut negative = 0_u64;
    let mut total = 0_u64;
    for (value, weight) in values {
        if value == 0 || weight == 0 {
            continue;
        }
        total = total.saturating_add(u64::from(weight));
        if value > 0 {
            positive = positive.saturating_add(u64::from(weight));
        } else {
            negative = negative.saturating_add(u64::from(weight));
        }
    }
    if total == 0 {
        0
    } else {
        positive.max(negative).saturating_mul(SCORE_SCALE) / total
    }
}

fn digest_input(output: &MechanismInvariance) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "context_order": output.context_order,
        "mechanism_order": output.mechanism_order,
        "signature_order": output.signature_order,
        "selected_signature_order": output.selected_signature_order,
        "quality_blocked_order": output.quality_blocked_order,
        "risk_blocked_order": output.risk_blocked_order,
        "context_blocked_order": output.context_blocked_order,
        "budget_blocked_order": output.budget_blocked_order,
        "pairs": output.pairs,
        "scores": output.scores,
        "utilities": output.utilities,
        "unresolved_pair_order": output.unresolved_pair_order,
        "budget_remaining_units": output.budget_remaining_units,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn validate_request(request: &MechanismInvarianceRequest) -> Result<(), MechanismInvarianceError> {
    if request.objective.trim().is_empty()
        || request.contexts.len() < 2
        || request.contexts.len() > MAX_CONTEXTS
        || request.mechanisms.len() < 2
        || request.mechanisms.len() > MAX_MECHANISMS
        || request.signatures.is_empty()
        || request.signatures.len() > MAX_SIGNATURES
        || request.min_contexts < 2
        || request.min_contexts > request.contexts.len()
        || request.invariance_floor_milli == 0
        || request.invariance_floor_milli > SCORE_SCALE
        || request.separation_floor_milli == 0
        || request.separation_floor_milli > SCORE_SCALE
        || request.min_signature_quality_milli > SCORE_SCALE as u16
        || request.risk_ceiling_milli > SCORE_SCALE as u16
        || request.budget_units == 0
        || request.max_selected_signatures == 0
        || request.max_selected_signatures > request.signatures.len()
    {
        return Err(MechanismInvarianceError::InvalidRequest(
            "objective, bounded contexts/mechanisms/signatures, positive floors, finite gates, budget, and selection limits are required".into(),
        ));
    }
    let mut context_ids = BTreeSet::new();
    let mut context_weight = 0_u32;
    for context in &request.contexts {
        if context.context_id.trim().is_empty()
            || context.label.trim().is_empty()
            || context.weight_milli == 0
            || !context_ids.insert(context.context_id.clone())
        {
            return Err(MechanismInvarianceError::InvalidInput(
                "context identities, labels, positive weights, and uniqueness are required".into(),
            ));
        }
        context_weight = context_weight.saturating_add(u32::from(context.weight_milli));
    }
    if context_weight != SCORE_SCALE as u32 {
        return Err(MechanismInvarianceError::InvalidInput(
            "context weights must sum to exactly 1000 milli-units".into(),
        ));
    }
    let mut mechanism_ids = BTreeSet::new();
    for mechanism in &request.mechanisms {
        if mechanism.mechanism_id.trim().is_empty()
            || mechanism.label.trim().is_empty()
            || !mechanism_ids.insert(mechanism.mechanism_id.clone())
        {
            return Err(MechanismInvarianceError::InvalidInput(
                "mechanism identities, labels, and uniqueness are required".into(),
            ));
        }
    }
    let mut signature_ids = BTreeSet::new();
    for signature in &request.signatures {
        if signature.signature_id.trim().is_empty()
            || signature.label.trim().is_empty()
            || signature.cost_units == 0
            || signature.quality_milli > SCORE_SCALE as u16
            || signature.risk_milli > SCORE_SCALE as u16
            || !signature_ids.insert(signature.signature_id.clone())
            || signature.value_milli_by_context_and_mechanism.len() != context_ids.len()
            || signature
                .value_milli_by_context_and_mechanism
                .keys()
                .any(|id| !context_ids.contains(id))
            || signature
                .value_milli_by_context_and_mechanism
                .values()
                .any(|values| {
                    values.len() != mechanism_ids.len()
                        || values.keys().any(|id| !mechanism_ids.contains(id))
                })
        {
            return Err(MechanismInvarianceError::InvalidInput(
                "signature identity, cost, quality/risk bounds, complete context/mechanism values, and uniqueness are required".into(),
            ));
        }
    }
    Ok(())
}

fn validate_output(output: &MechanismInvariance) -> Result<(), MechanismInvarianceError> {
    if output.feature_id != FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || !canonical(&output.context_order)
        || !canonical(&output.mechanism_order)
        || !canonical(&output.signature_order)
        || !canonical(&output.selected_signature_order)
        || !canonical(&output.quality_blocked_order)
        || !canonical(&output.risk_blocked_order)
        || !canonical(&output.context_blocked_order)
        || !canonical(&output.budget_blocked_order)
        || !canonical(&output.unresolved_pair_order)
        || !canonical(&output.negative_evidence)
        || !canonical(&output.uncertainty)
        || output
            .pairs
            .windows(2)
            .any(|pair| pair[0].pair_id >= pair[1].pair_id)
        || output
            .scores
            .windows(2)
            .any(|pair| pair[0].signature_id >= pair[1].signature_id)
        || output
            .utilities
            .windows(2)
            .any(|pair| pair[0].signature_id >= pair[1].signature_id)
        || output.scores.iter().any(|score| {
            score.context_count != score.context_order.len()
                || score.direction_consistency_milli > SCORE_SCALE
                || score.transport_stability_milli > SCORE_SCALE
                || score.invariance_score_milli > SCORE_SCALE
        })
        || output.pairs.iter().any(|pair| {
            pair.pair_id.trim().is_empty()
                || pair.left_mechanism_id.trim().is_empty()
                || pair.right_mechanism_id.trim().is_empty()
                || pair.separation_milli > SCORE_SCALE
                || pair.transport_stability_milli > SCORE_SCALE
                || !canonical(&pair.selected_signature_order)
                || pair.status.trim().is_empty()
        })
    {
        return Err(MechanismInvarianceError::InvalidOutput(
            "identity, canonical ordering, bounded scores, pair, signature, or status invariants are invalid".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| MechanismInvarianceError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(MechanismInvarianceError::InvalidOutput(
            "digest is not bound to the mechanism invariance frontier".into(),
        ));
    }
    Ok(())
}

impl MechanismInvariance {
    pub fn validate(&self) -> Result<(), MechanismInvarianceError> {
        validate_output(self)
    }
}

fn signature_pair_separation(
    signature: &MechanismSignature,
    contexts: &[MechanismInvarianceContext],
    left: &str,
    right: &str,
) -> u64 {
    let weighted = contexts.iter().fold(0_u64, |sum, context| {
        let values = signature
            .value_milli_by_context_and_mechanism
            .get(&context.context_id)
            .expect("validated context");
        let difference = values
            .get(left)
            .expect("validated mechanism")
            .abs_diff(*values.get(right).expect("validated mechanism"));
        sum.saturating_add(
            u64::from(difference.min(SCORE_SCALE as u32))
                .saturating_mul(u64::from(context.weight_milli)),
        )
    });
    weighted / SCORE_SCALE
}

fn signature_pair_stability(
    signature: &MechanismSignature,
    contexts: &[MechanismInvarianceContext],
    left: &str,
    right: &str,
) -> u64 {
    weighted_majority_consistency(contexts.iter().map(|context| {
        let values = signature
            .value_milli_by_context_and_mechanism
            .get(&context.context_id)
            .expect("validated context");
        let difference = i64::from(*values.get(left).expect("validated mechanism"))
            - i64::from(*values.get(right).expect("validated mechanism"));
        (difference, context.weight_milli)
    }))
}

fn score_signature(
    signature: &MechanismSignature,
    contexts: &[MechanismInvarianceContext],
    mechanisms: &[String],
) -> MechanismInvarianceSignatureScore {
    let context_order = contexts
        .iter()
        .map(|context| context.context_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let direction_consistency_milli = mechanisms
        .iter()
        .map(|mechanism_id| {
            weighted_majority_consistency(contexts.iter().map(|context| {
                let value = signature
                    .value_milli_by_context_and_mechanism
                    .get(&context.context_id)
                    .expect("validated context")
                    .get(mechanism_id)
                    .expect("validated mechanism");
                (i64::from(*value), context.weight_milli)
            }))
        })
        .sum::<u64>()
        / mechanisms.len() as u64;
    let pairs = mechanisms
        .iter()
        .enumerate()
        .flat_map(|(left_index, left)| {
            mechanisms
                .iter()
                .skip(left_index + 1)
                .map(move |right| (left, right))
        })
        .collect::<Vec<_>>();
    let (separation_milli, transport_stability_milli) = if pairs.is_empty() {
        (0, 0)
    } else {
        (
            pairs
                .iter()
                .map(|(left, right)| signature_pair_separation(signature, contexts, left, right))
                .sum::<u64>()
                / pairs.len() as u64,
            pairs
                .iter()
                .map(|(left, right)| signature_pair_stability(signature, contexts, left, right))
                .sum::<u64>()
                / pairs.len() as u64,
        )
    };
    let invariance_score_milli =
        direction_consistency_milli.saturating_mul(transport_stability_milli) / SCORE_SCALE;
    MechanismInvarianceSignatureScore {
        signature_id: signature.signature_id.clone(),
        label: signature.label.clone(),
        context_order,
        context_count: contexts.len(),
        direction_consistency_milli,
        separation_milli,
        transport_stability_milli,
        invariance_score_milli,
        selected: false,
    }
}

/// Score cross-model stability and select a bounded panel of invariant mechanistic signatures.
pub fn analyze_glioma_mechanism_invariance(
    request: &MechanismInvarianceRequest,
) -> Result<MechanismInvariance, MechanismInvarianceError> {
    validate_request(request)?;
    let context_order = request
        .contexts
        .iter()
        .map(|context| context.context_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mechanism_order = request
        .mechanisms
        .iter()
        .map(|mechanism| mechanism.mechanism_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let signature_order = request
        .signatures
        .iter()
        .map(|signature| signature.signature_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut quality_blocked_order = request
        .signatures
        .iter()
        .filter(|signature| signature.quality_milli < request.min_signature_quality_milli)
        .map(|signature| signature.signature_id.clone())
        .collect::<Vec<_>>();
    let mut risk_blocked_order = request
        .signatures
        .iter()
        .filter(|signature| signature.risk_milli > request.risk_ceiling_milli)
        .map(|signature| signature.signature_id.clone())
        .collect::<Vec<_>>();
    quality_blocked_order.sort();
    risk_blocked_order.sort();
    let eligible = request
        .signatures
        .iter()
        .filter(|signature| {
            signature.quality_milli >= request.min_signature_quality_milli
                && signature.risk_milli <= request.risk_ceiling_milli
        })
        .collect::<Vec<_>>();
    let mut context_blocked_order = Vec::new();
    if request.contexts.len() < request.min_contexts {
        context_blocked_order = context_order.clone();
    }
    let all_scores = request
        .signatures
        .iter()
        .map(|signature| score_signature(signature, &request.contexts, &mechanism_order))
        .collect::<Vec<_>>();
    let score_by_id = all_scores
        .iter()
        .map(|score| (score.signature_id.clone(), score.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut pair_records = Vec::new();
    for (left_index, left) in mechanism_order.iter().enumerate() {
        for right in mechanism_order.iter().skip(left_index + 1) {
            pair_records.push((pair_id(left, right), left.clone(), right.clone()));
        }
    }
    let mut selected = BTreeSet::new();
    let mut covered_pairs = BTreeSet::new();
    let mut spent = 0_u64;
    let mut utilities = Vec::new();
    let mut budget_blocked_order = Vec::new();
    loop {
        if selected.len() >= request.max_selected_signatures {
            break;
        }
        let mut best: Option<(u64, u64, String, Vec<String>)> = None;
        for signature in &eligible {
            if selected.contains(&signature.signature_id) {
                continue;
            }
            let projected_cost = spent.saturating_add(u64::from(signature.cost_units));
            if projected_cost > request.budget_units {
                budget_blocked_order.push(signature.signature_id.clone());
                continue;
            }
            let score = score_by_id
                .get(&signature.signature_id)
                .expect("score for validated signature");
            if score.context_count < request.min_contexts
                || score.invariance_score_milli < request.invariance_floor_milli
            {
                continue;
            }
            let mut gain = 0_u64;
            let mut covered = Vec::new();
            for (id, left, right) in &pair_records {
                if covered_pairs.contains(id) {
                    continue;
                }
                let separation =
                    signature_pair_separation(signature, &request.contexts, left, right);
                let stability = signature_pair_stability(signature, &request.contexts, left, right);
                if separation >= request.separation_floor_milli
                    && stability >= request.invariance_floor_milli
                {
                    gain = gain.saturating_add(separation.saturating_mul(stability) / SCORE_SCALE);
                    covered.push(id.clone());
                }
            }
            if gain == 0 {
                continue;
            }
            let candidate = (
                gain / u64::from(signature.cost_units),
                gain,
                signature.signature_id.clone(),
                covered,
            );
            if best
                .as_ref()
                .map(|current| {
                    candidate.0 > current.0
                        || (candidate.0 == current.0 && candidate.1 > current.1)
                        || (candidate.0 == current.0
                            && candidate.1 == current.1
                            && candidate.2 < current.2)
                })
                .unwrap_or(true)
            {
                best = Some(candidate);
            }
        }
        let Some((_, gain, signature_id, covered)) = best else {
            break;
        };
        let signature = eligible
            .iter()
            .find(|signature| signature.signature_id == signature_id)
            .expect("eligible signature");
        selected.insert(signature_id.clone());
        covered_pairs.extend(covered.iter().cloned());
        spent = spent.saturating_add(u64::from(signature.cost_units));
        utilities.push(MechanismInvarianceUtility {
            signature_id,
            marginal_value_milli: gain,
            covered_pair_order: covered,
            projected_cost_units: spent,
            action: "select-for-cross-model-invariance".into(),
        });
    }
    budget_blocked_order.sort();
    budget_blocked_order.dedup();
    utilities.sort_by(|left, right| left.signature_id.cmp(&right.signature_id));
    let selected_signature_order = selected.iter().cloned().collect::<Vec<_>>();
    let mut pairs = Vec::new();
    let mut unresolved_pair_order = Vec::new();
    for (id, left, right) in pair_records {
        let selected_ids = selected
            .iter()
            .filter(|signature_id| {
                let signature = request
                    .signatures
                    .iter()
                    .find(|item| &item.signature_id == *signature_id)
                    .expect("selected signature");
                signature_pair_separation(signature, &request.contexts, &left, &right)
                    >= request.separation_floor_milli
                    && signature_pair_stability(signature, &request.contexts, &left, &right)
                        >= request.invariance_floor_milli
            })
            .cloned()
            .collect::<Vec<_>>();
        let separation_milli = selected_ids
            .iter()
            .map(|signature_id| {
                request
                    .signatures
                    .iter()
                    .find(|item| &item.signature_id == signature_id)
                    .map(|signature| {
                        signature_pair_separation(signature, &request.contexts, &left, &right)
                    })
                    .unwrap_or(0)
            })
            .max()
            .unwrap_or(0);
        let transport_stability_milli = selected_ids
            .iter()
            .map(|signature_id| {
                request
                    .signatures
                    .iter()
                    .find(|item| &item.signature_id == signature_id)
                    .map(|signature| {
                        signature_pair_stability(signature, &request.contexts, &left, &right)
                    })
                    .unwrap_or(0)
            })
            .max()
            .unwrap_or(0);
        let status = if selected_ids.is_empty() {
            unresolved_pair_order.push(id.clone());
            "unresolved"
        } else {
            "invariant-signature-selected"
        };
        pairs.push(MechanismInvariancePair {
            pair_id: id,
            left_mechanism_id: left,
            right_mechanism_id: right,
            selected_signature_order: selected_ids,
            separation_milli,
            transport_stability_milli,
            status: status.into(),
        });
    }
    pairs.sort_by(|left, right| left.pair_id.cmp(&right.pair_id));
    unresolved_pair_order.sort();
    let mut scores = all_scores;
    for score in &mut scores {
        score.selected = selected.contains(&score.signature_id);
    }
    scores.sort_by(|left, right| left.signature_id.cmp(&right.signature_id));
    let mut negative_evidence = quality_blocked_order
        .iter()
        .map(|id| format!("quality-gate-blocked:{id}"))
        .chain(
            risk_blocked_order
                .iter()
                .map(|id| format!("risk-gate-blocked:{id}")),
        )
        .chain(
            context_blocked_order
                .iter()
                .map(|id| format!("context-gate-blocked:{id}")),
        )
        .chain(
            unresolved_pair_order
                .iter()
                .map(|id| format!("unresolved-pair:{id}")),
        )
        .collect::<Vec<_>>();
    negative_evidence.sort();
    let mut uncertainty = Vec::new();
    if !unresolved_pair_order.is_empty() {
        uncertainty.push("mechanism-pairs-lack-a-cross-model-invariant-signature".into());
    }
    if !budget_blocked_order.is_empty() {
        uncertainty.push("some-invariant-signatures-exceed-budget".into());
    }
    if eligible.iter().any(|signature| {
        score_by_id
            .get(&signature.signature_id)
            .is_some_and(|score| score.invariance_score_milli < request.invariance_floor_milli)
    }) {
        uncertainty.push("candidate-signatures-fail-invariance-floor".into());
    }
    uncertainty.sort();
    let disposition = if eligible.is_empty() {
        MechanismInvarianceDisposition::NoEligibleSignatures
    } else if unresolved_pair_order.is_empty() {
        MechanismInvarianceDisposition::Qualified
    } else if selected_signature_order.is_empty() && !budget_blocked_order.is_empty() {
        MechanismInvarianceDisposition::BudgetBlocked
    } else {
        MechanismInvarianceDisposition::Partial
    };
    let mut output = MechanismInvariance {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        context_order,
        mechanism_order,
        signature_order,
        selected_signature_order,
        quality_blocked_order,
        risk_blocked_order,
        context_blocked_order,
        budget_blocked_order,
        pairs,
        scores,
        utilities,
        unresolved_pair_order,
        budget_remaining_units: request.budget_units.saturating_sub(spent),
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"pending"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MechanismInvarianceError::Digest(error.to_string()))?;
    validate_output(&output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> MechanismInvarianceRequest {
        let contexts = vec![
            MechanismInvarianceContext {
                context_id: "organoid".into(),
                label: "organoid".into(),
                model_system: GliomaModelSystem::Organoid,
                weight_milli: 500,
            },
            MechanismInvarianceContext {
                context_id: "xenograft".into(),
                label: "xenograft".into(),
                model_system: GliomaModelSystem::PatientDerivedXenograft,
                weight_milli: 500,
            },
        ];
        let mechanisms = vec![
            InvarianceMechanism {
                mechanism_id: "m-a".into(),
                label: "matrix remodeling".into(),
            },
            InvarianceMechanism {
                mechanism_id: "m-b".into(),
                label: "immune mimicry".into(),
            },
        ];
        let stable = BTreeMap::from([
            (
                "organoid".into(),
                BTreeMap::from([("m-a".into(), 900), ("m-b".into(), 100)]),
            ),
            (
                "xenograft".into(),
                BTreeMap::from([("m-a".into(), 800), ("m-b".into(), 200)]),
            ),
        ]);
        let unstable = BTreeMap::from([
            (
                "organoid".into(),
                BTreeMap::from([("m-a".into(), 900), ("m-b".into(), 100)]),
            ),
            (
                "xenograft".into(),
                BTreeMap::from([("m-a".into(), 100), ("m-b".into(), 900)]),
            ),
        ]);
        MechanismInvarianceRequest {
            objective: "find transportable invasion signatures".into(),
            contexts,
            mechanisms,
            signatures: vec![
                MechanismSignature {
                    signature_id: "sig-stable".into(),
                    label: "stable invasion edge".into(),
                    value_milli_by_context_and_mechanism: stable,
                    quality_milli: 900,
                    cost_units: 2,
                    risk_milli: 100,
                },
                MechanismSignature {
                    signature_id: "sig-unstable".into(),
                    label: "context-reversing edge".into(),
                    value_milli_by_context_and_mechanism: unstable,
                    quality_milli: 900,
                    cost_units: 2,
                    risk_milli: 100,
                },
            ],
            min_contexts: 2,
            invariance_floor_milli: 700,
            separation_floor_milli: 400,
            min_signature_quality_milli: 700,
            risk_ceiling_milli: 500,
            budget_units: 8,
            max_selected_signatures: 2,
        }
    }

    #[test]
    fn selects_stable_signature_and_replays() {
        let first = analyze_glioma_mechanism_invariance(&request()).expect("frontier");
        let second = analyze_glioma_mechanism_invariance(&request()).expect("frontier");
        assert_eq!(first, second);
        assert_eq!(first.selected_signature_order, vec!["sig-stable"]);
        assert!(first.unresolved_pair_order.is_empty());
        first.validate().expect("valid output");
    }

    #[test]
    fn risk_gate_preserves_unresolved_pair() {
        let mut input = request();
        input.signatures[0].risk_milli = 900;
        let output = analyze_glioma_mechanism_invariance(&input).expect("frontier");
        assert_eq!(output.risk_blocked_order, vec!["sig-stable"]);
        assert_eq!(output.unresolved_pair_order, vec!["m-a__m-b"]);
        assert!(output
            .negative_evidence
            .iter()
            .any(|entry| entry == "unresolved-pair:m-a__m-b"));
    }
}
