//! The context compiler API and CLI surface (39.20).
//!
//! 39.20 describes four endpoints and a `bioprism context plan|compile|inspect|compare` CLI. There
//! is no service here and no CLI binary; what this module implements is the part of that
//! specification that is a *contract* rather than a transport — the request value, its content
//! hash, and the two refusals that make the surface safe.
//!
//! # Identical inputs produce identical plans
//!
//! 39.20's first invariant. [`ContextRequest::digest`] is a content hash over every field, and
//! [`plan`] is a pure function of a request and a candidate list, so two calls with equal inputs
//! produce equal plans including the ordering of the candidate trace. There is no clock, no
//! randomness and no map iteration order that could leak in.
//!
//! # Dry run never touches restricted data
//!
//! 39.20's third invariant. A [`ContextRequest`] in [`ResolutionDepth::DryRun`] that is handed a
//! restricted candidate returns [`CompilerApiError::DryRunTouchedRestrictedData`] rather than
//! quietly resolving it, because the whole purpose of a dry run is that it can be executed by
//! someone who is not entitled to the payload.
//!
//! # Comparison mode changes only the context policy
//!
//! 39.20's fourth invariant, and it is the same rule [`crate::ablation`] applies to a contrast:
//! comparing two compiles that differ in the policy *and* the budget attributes nothing to either.
//! [`compare`] refuses with the varied set named, and delegates nothing to statistics because there
//! are none to delegate to.
//!
//! # Not implemented
//!
//! No selection. [`plan`] partitions candidates into mandatory and optional, checks that the
//! envelope can afford the mandatory set, and stops. The value-per-token ordering of 39.07 and the
//! packing of 39.16 belong to `bioprism-obligation`'s budget controller and are not duplicated here.
//! No HTTP, no persistence, no CAS.

use crate::context::{estimates_are_comparable, NodeKind};
use crate::error::CompilerApiError;
use bioprism_ids::ContentHash;
use bioprism_obligation::TokenEstimate;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;

/// The resolution ladder a request asks for.
///
/// [`ResolutionDepth::DryRun`] is a depth rather than a flag so it cannot be combined with a depth
/// that would contradict it. A dry run resolves nothing; there is no "dry run at L3".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionDepth {
    /// Plan only: handles and counts, no payload, no restricted access.
    DryRun,
    /// Identity and locators only.
    L0,
    /// Computed views and summaries.
    L1,
    /// Views plus supporting detail.
    L2,
    /// Near-source detail for the selected subgraph.
    L3,
}

impl ResolutionDepth {
    pub fn is_dry_run(self) -> bool {
        matches!(self, ResolutionDepth::DryRun)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ResolutionDepth::DryRun => "dry_run",
            ResolutionDepth::L0 => "l0",
            ResolutionDepth::L1 => "l1",
            ResolutionDepth::L2 => "l2",
            ResolutionDepth::L3 => "l3",
        }
    }
}

/// The resource envelope a request declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenEnvelope {
    pub total: usize,
}

impl TokenEnvelope {
    pub fn tokens(total: usize) -> Self {
        TokenEnvelope { total }
    }
}

/// A candidate the caller offers the planner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanCandidate {
    pub node_id: String,
    pub kind: NodeKind,
    /// Part of the mandatory closure. Not tradeable for tokens.
    #[serde(default)]
    pub mandatory: bool,
    /// Resolving this candidate reads controlled data. A dry run must not.
    #[serde(default)]
    pub restricted: bool,
    pub estimate: TokenEstimate,
}

impl PlanCandidate {
    pub fn new(node_id: impl Into<String>, kind: NodeKind, estimate: TokenEstimate) -> Self {
        PlanCandidate {
            node_id: node_id.into(),
            kind,
            mandatory: false,
            restricted: false,
            estimate,
        }
    }

    pub fn mandatory(mut self) -> Self {
        self.mandatory = true;
        self
    }

    pub fn restricted(mut self) -> Self {
        self.restricted = true;
        self
    }
}

/// A compile request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextRequest {
    pub world_ref: String,
    pub decision_ref: String,
    pub role: String,
    pub policy_id: String,
    pub envelope: TokenEnvelope,
    pub depth: ResolutionDepth,
    /// Pinned, because 39.20's determinism invariant is conditioned on it.
    pub compiler_version: String,
}

impl ContextRequest {
    pub fn new(
        world_ref: impl Into<String>,
        decision_ref: impl Into<String>,
        role: impl Into<String>,
        policy_id: impl Into<String>,
        envelope: TokenEnvelope,
        depth: ResolutionDepth,
        compiler_version: impl Into<String>,
    ) -> Self {
        ContextRequest {
            world_ref: world_ref.into(),
            decision_ref: decision_ref.into(),
            role: role.into(),
            policy_id: policy_id.into(),
            envelope,
            depth,
            compiler_version: compiler_version.into(),
        }
    }

    /// Content hash of every field. The stable reference 39.20 requires an API to return.
    pub fn digest(&self) -> Result<String, CompilerApiError> {
        let value = json!({
            "world_ref": self.world_ref,
            "decision_ref": self.decision_ref,
            "role": self.role,
            "policy_id": self.policy_id,
            "envelope": self.envelope.total,
            "depth": self.depth.as_str(),
            "compiler_version": self.compiler_version,
        });
        ContentHash::of_value(&value)
            .map(|hash| hash.as_str().to_string())
            .map_err(|error| CompilerApiError::NotAddressable(error.to_string()))
    }

    /// Fields other than the context policy. Used by [`compare`] to enforce 39.20's fourth
    /// invariant.
    fn comparable_fields(&self) -> Vec<(&'static str, String)> {
        vec![
            ("world_ref", self.world_ref.clone()),
            ("decision_ref", self.decision_ref.clone()),
            ("role", self.role.clone()),
            ("envelope", self.envelope.total.to_string()),
            ("depth", self.depth.as_str().to_string()),
            ("compiler_version", self.compiler_version.clone()),
        ]
    }
}

/// A plan: what the compiler would consider, what it must include, and what that costs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextPlan {
    pub request_digest: String,
    pub plan_digest: String,
    /// Candidate ids in stable order.
    pub candidates: Vec<String>,
    /// The mandatory closure, which the envelope must afford.
    pub mandatory: BTreeSet<String>,
    /// Candidates a dry run reports as handles rather than resolving.
    pub handles: BTreeSet<String>,
    pub mandatory_estimate: TokenEstimate,
    pub optional_estimate: TokenEstimate,
    pub envelope: TokenEnvelope,
}

impl ContextPlan {
    /// Tokens left after the mandatory closure. `None` when the mandatory set alone overruns, which
    /// [`plan`] refuses before a plan is built, so this is `Some` on every value that exists.
    pub fn discretionary_tokens(&self) -> Option<usize> {
        self.envelope.total.checked_sub(self.mandatory_estimate.tokens)
    }
}

/// Build a plan.
///
/// Pure: the same request and the same candidates always produce the same plan, including the
/// candidate ordering, which is the caller's order preserved rather than a set iteration.
pub fn plan(
    request: &ContextRequest,
    candidates: &[PlanCandidate],
) -> Result<ContextPlan, CompilerApiError> {
    if request.depth.is_dry_run() {
        if let Some(candidate) = candidates.iter().find(|candidate| candidate.restricted) {
            return Err(CompilerApiError::DryRunTouchedRestrictedData(
                candidate.node_id.clone(),
            ));
        }
    }

    let mandatory: BTreeSet<String> = candidates
        .iter()
        .filter(|candidate| candidate.mandatory)
        .map(|candidate| candidate.node_id.clone())
        .collect();
    let mandatory_estimate = TokenEstimate::sum(
        candidates
            .iter()
            .filter(|candidate| candidate.mandatory)
            .map(|candidate| &candidate.estimate),
    );
    let optional_estimate = TokenEstimate::sum(
        candidates
            .iter()
            .filter(|candidate| !candidate.mandatory)
            .map(|candidate| &candidate.estimate),
    );

    if mandatory_estimate.tokens > request.envelope.total {
        return Err(CompilerApiError::EnvelopeBelowMandatoryClosure {
            envelope: request.envelope.total,
            mandatory: mandatory_estimate.tokens,
        });
    }

    let handles: BTreeSet<String> = if request.depth.is_dry_run() {
        candidates
            .iter()
            .map(|candidate| candidate.node_id.clone())
            .collect()
    } else {
        BTreeSet::new()
    };

    let request_digest = request.digest()?;
    let candidate_ids: Vec<String> = candidates
        .iter()
        .map(|candidate| candidate.node_id.clone())
        .collect();
    let plan_value = json!({
        "request": request_digest,
        "candidates": candidate_ids,
        "mandatory": mandatory.iter().cloned().collect::<Vec<_>>(),
        "mandatory_tokens": mandatory_estimate.tokens,
        "mandatory_estimator": mandatory_estimate.method.label(),
        "optional_tokens": optional_estimate.tokens,
    });
    let plan_digest = ContentHash::of_value(&plan_value)
        .map(|hash| hash.as_str().to_string())
        .map_err(|error| CompilerApiError::NotAddressable(error.to_string()))?;

    Ok(ContextPlan {
        request_digest,
        plan_digest,
        candidates: candidate_ids,
        mandatory,
        handles,
        mandatory_estimate,
        optional_estimate,
        envelope: request.envelope,
    })
}

/// What a comparison is allowed to vary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonMode {
    /// Only the context policy differs. The only mode 39.20 licenses.
    PolicyOnly,
}

/// Two plans compared under one varied policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyComparison {
    pub comparison_id: String,
    pub mode: ComparisonMode,
    pub baseline_policy: String,
    pub variant_policy: String,
    pub baseline_plan: ContextPlan,
    pub variant_plan: ContextPlan,
}

impl PolicyComparison {
    /// The mandatory-closure difference, when both were estimated the same way.
    ///
    /// `None` when the two are not comparable per [`crate::context::estimates_are_comparable`]. A
    /// difference between two rulers is not a difference in cost, and this is the same refusal
    /// [`crate::ablation::ContrastReport::token_difference`] makes.
    pub fn mandatory_difference(&self) -> Option<i64> {
        if !estimates_are_comparable(
            &self.baseline_plan.mandatory_estimate,
            &self.variant_plan.mandatory_estimate,
        ) {
            return None;
        }
        Some(
            self.variant_plan.mandatory_estimate.tokens as i64
                - self.baseline_plan.mandatory_estimate.tokens as i64,
        )
    }

    /// Nodes the variant policy admits to the mandatory closure that the baseline does not.
    pub fn mandatory_added(&self) -> BTreeSet<String> {
        self.variant_plan
            .mandatory
            .difference(&self.baseline_plan.mandatory)
            .cloned()
            .collect()
    }

    /// Nodes the variant policy drops from the mandatory closure. The interesting direction: a
    /// policy that saves tokens by shrinking the mandatory set is not saving tokens.
    pub fn mandatory_removed(&self) -> BTreeSet<String> {
        self.baseline_plan
            .mandatory
            .difference(&self.variant_plan.mandatory)
            .cloned()
            .collect()
    }
}

/// Compare two compiles, refusing when they vary more than the context policy.
pub fn compare(
    comparison_id: impl Into<String>,
    baseline: &ContextRequest,
    variant: &ContextRequest,
    baseline_candidates: &[PlanCandidate],
    variant_candidates: &[PlanCandidate],
) -> Result<PolicyComparison, CompilerApiError> {
    let comparison_id = comparison_id.into();
    let varied: Vec<String> = baseline
        .comparable_fields()
        .into_iter()
        .zip(variant.comparable_fields())
        .filter(|((_, left), (_, right))| left != right)
        .map(|((name, _), _)| name.to_string())
        .collect();
    if !varied.is_empty() {
        return Err(CompilerApiError::ComparisonVariesMoreThanPolicy {
            comparison: comparison_id,
            varied,
        });
    }
    if baseline.policy_id == variant.policy_id {
        return Err(CompilerApiError::ComparisonVariesNothing {
            comparison: comparison_id,
            policy: baseline.policy_id.clone(),
        });
    }
    Ok(PolicyComparison {
        comparison_id,
        mode: ComparisonMode::PolicyOnly,
        baseline_policy: baseline.policy_id.clone(),
        variant_policy: variant.policy_id.clone(),
        baseline_plan: plan(baseline, baseline_candidates)?,
        variant_plan: plan(variant, variant_candidates)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn est(tokens: usize) -> TokenEstimate {
        TokenEstimate::declared(tokens)
    }

    fn request(policy: &str, depth: ResolutionDepth, envelope: usize) -> ContextRequest {
        ContextRequest::new(
            "world/glioma-2026",
            "decision/board",
            "molecular",
            policy,
            TokenEnvelope::tokens(envelope),
            depth,
            "compiler/1.0.0",
        )
    }

    fn candidates() -> Vec<PlanCandidate> {
        vec![
            PlanCandidate::new("n/identity", NodeKind::Invariant, est(20)).mandatory(),
            PlanCandidate::new("n/units", NodeKind::Invariant, est(10)).mandatory(),
            PlanCandidate::new("n/expr", NodeKind::Evidence, est(400)),
            PlanCandidate::new("n/conflict", NodeKind::Contradiction, est(60)).mandatory(),
        ]
    }

    #[test]
    fn identical_requests_under_a_pinned_compiler_produce_identical_plans() {
        let first = plan(&request("policy/a", ResolutionDepth::L1, 5000), &candidates())
            .expect("plans");
        let second = plan(&request("policy/a", ResolutionDepth::L1, 5000), &candidates())
            .expect("plans");
        assert_eq!(first, second);
        assert_eq!(first.plan_digest, second.plan_digest);
    }

    #[test]
    fn a_different_policy_produces_a_different_request_digest() {
        let left = request("policy/a", ResolutionDepth::L1, 5000);
        let right = request("policy/b", ResolutionDepth::L1, 5000);
        assert_ne!(
            left.digest().expect("digests"),
            right.digest().expect("digests")
        );
    }

    #[test]
    fn an_envelope_below_the_mandatory_closure_is_refused_rather_than_trimmed() {
        let result = plan(&request("policy/a", ResolutionDepth::L1, 50), &candidates());
        assert!(matches!(
            result,
            Err(CompilerApiError::EnvelopeBelowMandatoryClosure {
                envelope: 50,
                mandatory: 90
            })
        ));
    }

    #[test]
    fn a_dry_run_refuses_to_resolve_a_restricted_candidate() {
        let mut restricted = candidates();
        restricted.push(PlanCandidate::new("n/raw", NodeKind::Handle, est(9000)).restricted());
        assert!(matches!(
            plan(&request("policy/a", ResolutionDepth::DryRun, 50_000), &restricted),
            Err(CompilerApiError::DryRunTouchedRestrictedData(ref node)) if node == "n/raw"
        ));
    }

    #[test]
    fn a_dry_run_returns_every_candidate_as_a_handle() {
        let plan = plan(
            &request("policy/a", ResolutionDepth::DryRun, 5000),
            &candidates(),
        )
        .expect("plans");
        assert_eq!(plan.handles.len(), 4);
        assert!(plan.handles.contains("n/expr"));
    }

    #[test]
    fn a_resolving_depth_returns_no_handles() {
        let plan = plan(&request("policy/a", ResolutionDepth::L2, 5000), &candidates())
            .expect("plans");
        assert!(plan.handles.is_empty());
    }

    #[test]
    fn a_comparison_that_varies_the_budget_as_well_as_the_policy_is_refused() {
        let result = compare(
            "cmp/1",
            &request("policy/a", ResolutionDepth::L1, 5000),
            &request("policy/b", ResolutionDepth::L1, 2000),
            &candidates(),
            &candidates(),
        );
        assert!(matches!(
            result,
            Err(CompilerApiError::ComparisonVariesMoreThanPolicy { ref varied, .. })
                if varied == &vec!["envelope".to_string()]
        ));
    }

    #[test]
    fn a_comparison_that_varies_nothing_is_refused() {
        let result = compare(
            "cmp/1",
            &request("policy/a", ResolutionDepth::L1, 5000),
            &request("policy/a", ResolutionDepth::L1, 5000),
            &candidates(),
            &candidates(),
        );
        assert!(matches!(
            result,
            Err(CompilerApiError::ComparisonVariesNothing { .. })
        ));
    }

    #[test]
    fn a_policy_that_shrinks_the_mandatory_closure_is_visible_as_a_removal_not_a_saving() {
        let mut lenient = candidates();
        lenient[3].mandatory = false;
        let comparison = compare(
            "cmp/1",
            &request("policy/strict", ResolutionDepth::L1, 5000),
            &request("policy/lenient", ResolutionDepth::L1, 5000),
            &candidates(),
            &lenient,
        )
        .expect("compares");
        assert_eq!(comparison.mandatory_difference(), Some(-60));
        assert!(comparison.mandatory_removed().contains("n/conflict"));
        assert!(comparison.mandatory_added().is_empty());
    }

    #[test]
    fn a_mandatory_difference_across_estimators_is_not_reported() {
        let mut retokenized = candidates();
        for candidate in &mut retokenized {
            candidate.estimate = TokenEstimate::from_provider(candidate.estimate.tokens, "cl100k");
        }
        let comparison = compare(
            "cmp/1",
            &request("policy/a", ResolutionDepth::L1, 5000),
            &request("policy/b", ResolutionDepth::L1, 5000),
            &candidates(),
            &retokenized,
        )
        .expect("compares");
        assert_eq!(comparison.mandatory_difference(), None);
    }

    #[test]
    fn a_plan_reports_what_is_left_after_the_mandatory_closure() {
        let plan = plan(&request("policy/a", ResolutionDepth::L1, 500), &candidates())
            .expect("plans");
        assert_eq!(plan.discretionary_tokens(), Some(410));
    }

    #[test]
    fn a_plan_survives_a_json_round_trip() {
        let plan = plan(&request("policy/a", ResolutionDepth::L1, 5000), &candidates())
            .expect("plans");
        let text = serde_json::to_string(&plan).expect("serialises");
        let back: ContextPlan = serde_json::from_str(&text).expect("deserialises");
        assert_eq!(back, plan);
    }
}
