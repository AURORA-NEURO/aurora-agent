//! Golden context fixtures and drift diagnosis (39.21).
//!
//! Blueprint 39.21 asks for "a conformance suite that catches biologically dangerous compression
//! defects". This workspace's parity story already rests on golden artifacts everywhere else; this
//! module is the *context* equivalent, and its job is to make a change that silently degrades a
//! compiled context fail a test instead of passing quietly.
//!
//! # Why the report is the product, not the verdict
//!
//! A fixture that says "digest differs" has told a developer nothing they can act on. The whole
//! value of this module is [`ContextDrift`]: a typed list of *what* moved, each entry naming the
//! node, obligation, slot or omission group involved, and each carrying a
//! [`DriftSeverity`] that says whether it is a defect, a regression, or wording. The verdict is a
//! fold over that list, computed last and derivable by anyone.
//!
//! # The four invariants of 39.21, as code
//!
//! 1. *"Golden expected context is set-valued where multiple projections are valid."* A
//!    [`ContextFixture`] holds a list of [`ContextExpectation`]s and accepts a compile that matches
//!    **any** of them. When none matches it reports drift against the *closest*, named, so the
//!    developer is not left diffing against an arbitrary arm.
//! 2. *"Tests assert semantics, not exact prose."* A [`ContextExpectation`] stores node ids, kinds,
//!    obligations, invariant slots, omission influence classes, sufficiency and a token band. It
//!    can also pin a rendering digest, and a rendering change is always
//!    [`DriftSeverity::Advisory`] and never fails a check. The named failure mode of 39.21 is
//!    "brittle prose snapshot", so wording is observable and non-fatal by construction.
//! 3. *"Every modality has malformed and adversarial fixtures."* [`FixtureBundle::coverage_gaps`]
//!    enumerates the missing ones by name; [`FixtureBundle::validate`] turns them into a typed
//!    error.
//! 4. *"Fixtures include contradictions and failed assays."* Bundle-level requirements, checked the
//!    same way.
//!
//! # Fixture leakage is refused at pin time
//!
//! 39.21 lists "fixture leakage" as a failure mode: a golden that pins the evaluator's hidden
//! answer teaches the compiler to emit it. [`ContextFixture::pin`] therefore returns
//! [`FixtureError::HoldoutLeakedIntoFixture`] rather than skipping such a node, because skipping
//! would produce a fixture that quietly disagrees with the compile it was taken from.
//!
//! # Tokenizer drift is a defect, not a number change
//!
//! [`ContextDrift::EstimatorChanged`] is [`DriftSeverity::Critical`]. This is stronger than it
//! looks: when the estimator changes, every token comparison in the report is between two different
//! rulers, so the token band is not "violated", it is *meaningless*. Reporting that as a small
//! regression would be the exact fabrication `bioprism-docgraph` refused when it separated an
//! estimate from a measurement.
//!
//! # Not implemented
//!
//! No fixture serialisation format on disk, no test-runner integration, no minimisation of a
//! failing case to a smallest reproducer. 39.21's "reproducible fixture bundle" is modelled as a
//! value with a content hash; writing it to a repository layout is a harness concern.

use crate::context::{CompiledContext, ContextNode, NodeKind};
use crate::error::FixtureError;
use bioprism_ids::ContentHash;
use bioprism_obligation::{EstimationMethod, SufficiencyStatus, TokenEstimate};
use bioprism_section::InfluenceClass;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

/// The token interval a compile is expected to land in.
///
/// A band rather than an exact number, because the number is an estimate and pinning an estimate to
/// the unit would make every harmless reordering a failure. The band is inclusive at both ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenBand {
    pub floor: usize,
    pub ceiling: usize,
}

impl TokenBand {
    pub fn new(floor: usize, ceiling: usize) -> Self {
        TokenBand {
            floor: floor.min(ceiling),
            ceiling: floor.max(ceiling),
        }
    }

    /// A band centred on an observed estimate with a percentage tolerance either side.
    ///
    /// Integer arithmetic throughout: a fixture that produced a different band on a different
    /// machine's float rounding would defeat the purpose of having one.
    pub fn around(tokens: usize, tolerance_percent: usize) -> Self {
        let slack = tokens.saturating_mul(tolerance_percent) / 100;
        TokenBand {
            floor: tokens.saturating_sub(slack),
            ceiling: tokens.saturating_add(slack),
        }
    }

    pub fn contains(&self, tokens: usize) -> bool {
        tokens >= self.floor && tokens <= self.ceiling
    }
}

/// What a golden expects of one node.
///
/// Structural facts only. There is no field here that could hold rendered text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeExpectation {
    pub node_id: String,
    pub kind: NodeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obligation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invariant_slot: Option<String>,
    /// Whether the node must carry a source locator. 39.13 and 39.14 require one on derived views
    /// and claims; ordinary evidence often has one too, and a fixture says which it insists on.
    #[serde(default)]
    pub locator_required: bool,
}

impl NodeExpectation {
    fn of(node: &ContextNode) -> Self {
        NodeExpectation {
            node_id: node.node_id.clone(),
            kind: node.kind,
            obligation: node.obligation.clone(),
            invariant_slot: node.invariant_slot.clone(),
            locator_required: node.locator.is_some(),
        }
    }
}

/// One acceptable compiled projection.
///
/// A fixture holds several of these because 39.21's first invariant says a golden is set-valued
/// where more than one projection is valid — two orderings of an equal-value frontier, or two
/// equally good summarisations, are both correct and a golden that picks one is a golden that
/// fails on a legal change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextExpectation {
    pub label: String,
    pub policy_id: String,
    pub nodes: Vec<NodeExpectation>,
    /// Reject nodes the expectation does not list. Off by default: an extra node is usually a
    /// compiler being more generous, which is worth reporting but not worth failing.
    #[serde(default)]
    pub closed: bool,
    pub required_invariant_slots: BTreeSet<String>,
    pub required_obligations: BTreeSet<String>,
    /// Minimum count per protected kind. The defect this catches is a packer that dropped the
    /// second of two contradictions and left the first, which no per-node expectation notices.
    #[serde(default)]
    pub minimum_kind_counts: BTreeMap<NodeKind, usize>,
    pub sufficiency: SufficiencyStatus,
    /// Expected influence class per omission reason. `Zero` degrading to `Unknown` is the single
    /// most dangerous silent change a context compiler can make, and it has a variant of its own.
    #[serde(default)]
    pub omission_influence: BTreeMap<String, InfluenceClass>,
    pub token_band: TokenBand,
    /// The estimator the band was measured with. Comparing across estimators is refused.
    pub estimator: EstimationMethod,
    /// Optional rendering digests, per node. Always advisory.
    #[serde(default)]
    pub renderings: BTreeMap<String, String>,
}

impl ContextExpectation {
    fn node(&self, node_id: &str) -> Option<&NodeExpectation> {
        self.nodes.iter().find(|node| node.node_id == node_id)
    }
}

/// How much a single drift matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftSeverity {
    /// Observable, not a defect. Wording moved; the semantics did not.
    Advisory,
    /// The context got worse in a way nobody declared. Fails the check.
    Regression,
    /// A protected class, an invariant, an obligation, or the meaning of a token number moved.
    Critical,
}

/// One named difference between a golden and a compile.
///
/// Every variant carries the operand a developer would go and look at. That is the point: the
/// module's value is not the boolean.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "drift", rename_all = "snake_case")]
pub enum ContextDrift {
    /// A node the golden requires is absent from the compile.
    NodeMissing {
        node_id: String,
        kind: NodeKind,
        obligation: Option<String>,
    },
    /// The compile selected a node the golden does not list.
    NodeUnexpected { node_id: String, kind: NodeKind },
    /// A node is present but reclassified. The polarity defect: a contradiction rendered as
    /// ordinary evidence is still in the context and no longer reads as a conflict.
    NodeKindChanged {
        node_id: String,
        expected: NodeKind,
        actual: NodeKind,
    },
    /// A node no longer claims to serve the obligation it was selected for.
    NodeObligationChanged {
        node_id: String,
        expected: Option<String>,
        actual: Option<String>,
    },
    /// A node lost the locator that made it expandable back to source.
    LocatorLost { node_id: String },
    /// No selected node serves an obligation the golden requires covered.
    ObligationCoverageLost { obligation: String },
    /// A non-compressible invariant slot is unfilled.
    InvariantSlotUnfilled { slot: String },
    /// Fewer nodes of a protected kind than the golden requires.
    ProtectedKindCountFell {
        kind: NodeKind,
        expected_at_least: usize,
        actual: usize,
    },
    /// An omission group's influence claim got weaker: something that provably could not matter is
    /// now something nobody checked.
    OmissionInfluenceWeakened {
        reason: String,
        expected: InfluenceClass,
        actual: InfluenceClass,
    },
    /// An omission group's influence claim got *stronger* than the golden reviewed. Not obviously
    /// wrong, and not obviously right either: an argument the golden never saw is now load-bearing.
    OmissionInfluenceStrengthened {
        reason: String,
        expected: InfluenceClass,
        actual: InfluenceClass,
    },
    /// A whole omission group vanished from the manifest.
    OmissionGroupDisappeared { reason: String },
    /// A group appeared that the golden does not describe.
    OmissionGroupAppeared {
        reason: String,
        influence: InfluenceClass,
    },
    /// The sufficiency certificate is weaker than the golden's.
    SufficiencyWeakened {
        expected: SufficiencyStatus,
        actual: SufficiencyStatus,
    },
    /// The sufficiency certificate is stronger than the golden's, on evidence the golden did not
    /// contain. A compiler that started claiming sufficiency is a change that must be looked at.
    SufficiencyStrengthened {
        expected: SufficiencyStatus,
        actual: SufficiencyStatus,
    },
    /// The estimated total left the golden's band.
    TokenTotalOutsideBand { band: TokenBand, actual: usize },
    /// The token numbers were produced by a different rule. Every cost comparison in this report is
    /// between two rulers and none of it means anything until the band is re-derived.
    EstimatorChanged {
        expected: String,
        actual: String,
    },
    /// The compile ran under a different context policy than the golden pinned.
    PolicyChanged { expected: String, actual: String },
    /// Rendered wording changed. Never fatal.
    RenderingChanged { node_id: String },
}

impl ContextDrift {
    pub fn severity(&self) -> DriftSeverity {
        match self {
            ContextDrift::RenderingChanged { .. } => DriftSeverity::Advisory,
            ContextDrift::NodeUnexpected { .. }
            | ContextDrift::OmissionGroupAppeared { .. }
            | ContextDrift::OmissionInfluenceStrengthened { .. }
            | ContextDrift::SufficiencyStrengthened { .. }
            | ContextDrift::TokenTotalOutsideBand { .. }
            | ContextDrift::LocatorLost { .. }
            | ContextDrift::OmissionGroupDisappeared { .. } => DriftSeverity::Regression,
            ContextDrift::NodeMissing { .. }
            | ContextDrift::NodeKindChanged { .. }
            | ContextDrift::NodeObligationChanged { .. }
            | ContextDrift::ObligationCoverageLost { .. }
            | ContextDrift::InvariantSlotUnfilled { .. }
            | ContextDrift::ProtectedKindCountFell { .. }
            | ContextDrift::OmissionInfluenceWeakened { .. }
            | ContextDrift::SufficiencyWeakened { .. }
            | ContextDrift::EstimatorChanged { .. }
            | ContextDrift::PolicyChanged { .. } => DriftSeverity::Critical,
        }
    }

    /// A one-line diagnosis naming the thing to go and look at.
    pub fn describe(&self) -> String {
        match self {
            ContextDrift::NodeMissing {
                node_id,
                kind,
                obligation,
            } => match obligation {
                Some(obligation) => format!(
                    "node `{node_id}` ({}) serving obligation `{obligation}` is no longer selected",
                    kind.as_str()
                ),
                None => format!("node `{node_id}` ({}) is no longer selected", kind.as_str()),
            },
            ContextDrift::NodeUnexpected { node_id, kind } => format!(
                "node `{node_id}` ({}) was selected and the golden does not list it",
                kind.as_str()
            ),
            ContextDrift::NodeKindChanged {
                node_id,
                expected,
                actual,
            } => format!(
                "node `{node_id}` is now {} where the golden recorded {}",
                actual.as_str(),
                expected.as_str()
            ),
            ContextDrift::NodeObligationChanged {
                node_id,
                expected,
                actual,
            } => format!(
                "node `{node_id}` now serves {} where the golden recorded {}",
                actual.as_deref().unwrap_or("no obligation"),
                expected.as_deref().unwrap_or("no obligation")
            ),
            ContextDrift::LocatorLost { node_id } => {
                format!("node `{node_id}` lost its source locator and can no longer be expanded")
            }
            ContextDrift::ObligationCoverageLost { obligation } => {
                format!("no selected node serves obligation `{obligation}`")
            }
            ContextDrift::InvariantSlotUnfilled { slot } => {
                format!("non-compressible invariant slot `{slot}` is unfilled")
            }
            ContextDrift::ProtectedKindCountFell {
                kind,
                expected_at_least,
                actual,
            } => format!(
                "{} nodes fell from at least {expected_at_least} to {actual}",
                kind.as_str()
            ),
            ContextDrift::OmissionInfluenceWeakened {
                reason,
                expected,
                actual,
            } => format!(
                "omission group `{reason}` weakened from {} to {}: what was argued is now unchecked",
                expected.as_str(),
                actual.as_str()
            ),
            ContextDrift::OmissionInfluenceStrengthened {
                reason,
                expected,
                actual,
            } => format!(
                "omission group `{reason}` strengthened from {} to {} on an argument the golden never reviewed",
                expected.as_str(),
                actual.as_str()
            ),
            ContextDrift::OmissionGroupDisappeared { reason } => {
                format!("omission group `{reason}` is no longer reported")
            }
            ContextDrift::OmissionGroupAppeared { reason, influence } => format!(
                "new omission group `{reason}` with influence {}",
                influence.as_str()
            ),
            ContextDrift::SufficiencyWeakened { expected, actual } => format!(
                "sufficiency weakened from {} to {}",
                expected.as_str(),
                actual.as_str()
            ),
            ContextDrift::SufficiencyStrengthened { expected, actual } => format!(
                "sufficiency strengthened from {} to {}; the stronger claim was not in the golden",
                expected.as_str(),
                actual.as_str()
            ),
            ContextDrift::TokenTotalOutsideBand { band, actual } => format!(
                "estimated total {actual} is outside the band {}..={}",
                band.floor, band.ceiling
            ),
            ContextDrift::EstimatorChanged { expected, actual } => format!(
                "token estimator changed from `{expected}` to `{actual}`; the band and every cost \
                 comparison in this report are between different rulers"
            ),
            ContextDrift::PolicyChanged { expected, actual } => {
                format!("context policy changed from `{expected}` to `{actual}`")
            }
            ContextDrift::RenderingChanged { node_id } => {
                format!("rendered wording of node `{node_id}` changed; semantics unaffected")
            }
        }
    }
}

/// The outcome of checking a compile against a golden.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureVerdict {
    /// Some accepted projection matched exactly.
    Accepted,
    /// Some accepted projection matched apart from wording.
    AcceptedWithAdvisories,
    /// No accepted projection matched without a regression or a defect.
    Rejected,
}

impl FixtureVerdict {
    pub fn is_accepted(self) -> bool {
        matches!(
            self,
            FixtureVerdict::Accepted | FixtureVerdict::AcceptedWithAdvisories
        )
    }
}

/// Drift against one candidate expectation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExpectationComparison {
    pub label: String,
    pub drifts: Vec<ContextDrift>,
}

impl ExpectationComparison {
    pub fn count_at(&self, severity: DriftSeverity) -> usize {
        self.drifts
            .iter()
            .filter(|drift| drift.severity() == severity)
            .count()
    }

    fn rank(&self) -> (usize, usize, usize) {
        (
            self.count_at(DriftSeverity::Critical),
            self.count_at(DriftSeverity::Regression),
            self.drifts.len(),
        )
    }

    fn verdict(&self) -> FixtureVerdict {
        if self.drifts.is_empty() {
            FixtureVerdict::Accepted
        } else if self.rank().0 == 0 && self.rank().1 == 0 {
            FixtureVerdict::AcceptedWithAdvisories
        } else {
            FixtureVerdict::Rejected
        }
    }
}

/// What a fixture check produced.
///
/// Carries the per-expectation comparisons as well as the chosen one, so a developer looking at a
/// set-valued golden can see that arm B was nearly right rather than only that arm A was wrong.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FixtureReport {
    pub fixture_id: String,
    pub verdict: FixtureVerdict,
    /// The expectation the verdict was taken from: the matching one, or the closest.
    pub against: String,
    pub drifts: Vec<ContextDrift>,
    pub all: Vec<ExpectationComparison>,
}

impl FixtureReport {
    pub fn critical(&self) -> impl Iterator<Item = &ContextDrift> {
        self.drifts
            .iter()
            .filter(|drift| drift.severity() == DriftSeverity::Critical)
    }

    pub fn regressions(&self) -> impl Iterator<Item = &ContextDrift> {
        self.drifts
            .iter()
            .filter(|drift| drift.severity() == DriftSeverity::Regression)
    }

    pub fn advisories(&self) -> impl Iterator<Item = &ContextDrift> {
        self.drifts
            .iter()
            .filter(|drift| drift.severity() == DriftSeverity::Advisory)
    }

    /// Every drift as a line a developer can act on, ordered defects first.
    pub fn diagnosis(&self) -> Vec<String> {
        let mut lines: Vec<(DriftSeverity, String)> = self
            .drifts
            .iter()
            .map(|drift| (drift.severity(), drift.describe()))
            .collect();
        lines.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
        lines.into_iter().map(|(_, line)| line).collect()
    }
}

/// A golden context fixture: a decision, and the set of projections that are acceptable for it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextFixture {
    pub fixture_id: String,
    pub decision_ref: String,
    pub accepted: Vec<ContextExpectation>,
    /// Nodes the world marks as evaluator holdouts. Declared on the fixture so
    /// [`ContextFixture::validate`] can refuse a golden that pins one.
    #[serde(default)]
    pub holdout_nodes: BTreeSet<String>,
}

impl ContextFixture {
    pub fn new(fixture_id: impl Into<String>, decision_ref: impl Into<String>) -> Self {
        ContextFixture {
            fixture_id: fixture_id.into(),
            decision_ref: decision_ref.into(),
            accepted: Vec::new(),
            holdout_nodes: BTreeSet::new(),
        }
    }

    pub fn accepting(mut self, expectation: ContextExpectation) -> Self {
        self.accepted.push(expectation);
        self
    }

    pub fn with_holdouts<I, S>(mut self, nodes: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.holdout_nodes.extend(nodes.into_iter().map(Into::into));
        self
    }

    /// Derive an expectation from a real compile.
    ///
    /// This is the constructor a developer uses: run the reference compiler, pin what it produced,
    /// review the pin. `tolerance_percent` widens the token band, because the number being pinned is
    /// an estimate and pinning an estimate exactly makes a fixture that fails on rounding.
    ///
    /// Refuses a context containing evaluator holdout state. Skipping such nodes instead would
    /// produce a golden that silently disagrees with the compile it came from, and the next
    /// developer would spend an afternoon on the difference.
    pub fn pin(
        fixture_id: &str,
        label: impl Into<String>,
        context: &CompiledContext,
        tolerance_percent: usize,
    ) -> Result<ContextExpectation, FixtureError> {
        if let Some(node) = context.nodes.iter().find(|node| node.visibility.is_holdout()) {
            return Err(FixtureError::HoldoutLeakedIntoFixture {
                fixture: fixture_id.to_string(),
                node: node.node_id.clone(),
            });
        }
        let total = context.total_estimate();
        let mut minimum_kind_counts = BTreeMap::new();
        for kind in [
            NodeKind::Invariant,
            NodeKind::Contradiction,
            NodeKind::NegativeEvidence,
            NodeKind::Uncertainty,
            NodeKind::PolicyRestriction,
        ] {
            let count = context.count_of_kind(kind);
            if count > 0 {
                minimum_kind_counts.insert(kind, count);
            }
        }
        let renderings = context
            .nodes
            .iter()
            .filter_map(|node| {
                node.rendering
                    .as_ref()
                    .map(|digest| (node.node_id.clone(), digest.clone()))
            })
            .collect();
        let omission_influence = context
            .omissions
            .groups
            .iter()
            .map(|group| (group.reason.clone(), group.influence))
            .collect();
        Ok(ContextExpectation {
            label: label.into(),
            policy_id: context.policy_id.clone(),
            nodes: context.nodes.iter().map(NodeExpectation::of).collect(),
            closed: false,
            required_invariant_slots: context.filled_invariant_slots(),
            required_obligations: context.covered_obligations(),
            minimum_kind_counts,
            sufficiency: context.sufficiency,
            omission_influence,
            token_band: TokenBand::around(total.tokens, tolerance_percent),
            estimator: total.method,
            renderings,
        })
    }

    /// Structural checks on the golden itself, before it is used to judge anything.
    pub fn validate(&self) -> Result<(), FixtureError> {
        if self.accepted.is_empty() {
            return Err(FixtureError::NoAcceptedProjections(self.fixture_id.clone()));
        }
        let mut seen = BTreeSet::new();
        for expectation in &self.accepted {
            if !seen.insert(expectation.label.clone()) {
                return Err(FixtureError::DuplicateExpectation {
                    fixture: self.fixture_id.clone(),
                    expectation: expectation.label.clone(),
                });
            }
            for node in &expectation.nodes {
                if self.holdout_nodes.contains(&node.node_id) {
                    return Err(FixtureError::HoldoutLeakedIntoFixture {
                        fixture: self.fixture_id.clone(),
                        node: node.node_id.clone(),
                    });
                }
            }
            for node_id in expectation.renderings.keys() {
                if self.holdout_nodes.contains(node_id) {
                    return Err(FixtureError::HoldoutLeakedIntoFixture {
                        fixture: self.fixture_id.clone(),
                        node: node_id.clone(),
                    });
                }
                if expectation.node(node_id).is_none() {
                    return Err(FixtureError::RenderingPinnedWithoutSemantics {
                        fixture: self.fixture_id.clone(),
                        node: node_id.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Content hash of the golden, so a fixture bundle can be pinned the way any other artifact is.
    pub fn digest(&self) -> Result<String, FixtureError> {
        let value = serde_json::to_value(self).map_err(|error| {
            FixtureError::NotAddressable(self.fixture_id.clone(), error.to_string())
        })?;
        ContentHash::of_value(&value)
            .map(|hash| hash.as_str().to_string())
            .map_err(|error| {
                FixtureError::NotAddressable(self.fixture_id.clone(), error.to_string())
            })
    }

    /// Check a compile against every accepted projection and report the best outcome.
    ///
    /// When several match, the first in declaration order wins; when none does, the closest is the
    /// one with fewest critical drifts, then fewest regressions, then fewest drifts, then earliest
    /// declared. Fully deterministic, because a fixture that reported a different closest arm on a
    /// different run would be worse than no report.
    pub fn check(&self, context: &CompiledContext) -> Result<FixtureReport, FixtureError> {
        self.validate()?;
        let all: Vec<ExpectationComparison> = self
            .accepted
            .iter()
            .map(|expectation| ExpectationComparison {
                label: expectation.label.clone(),
                drifts: compare(expectation, context),
            })
            .collect();
        let best = all
            .iter()
            .enumerate()
            .min_by_key(|(index, comparison)| (comparison.rank(), *index))
            .map(|(_, comparison)| comparison)
            .expect("validate rejects an empty accepted set");
        Ok(FixtureReport {
            fixture_id: self.fixture_id.clone(),
            verdict: best.verdict(),
            against: best.label.clone(),
            drifts: best.drifts.clone(),
            all,
        })
    }
}

fn influence_rank(class: InfluenceClass) -> u8 {
    match class {
        InfluenceClass::Zero => 4,
        InfluenceClass::Bounded => 3,
        InfluenceClass::DeferredAcquisition => 2,
        InfluenceClass::InaccessibleByPolicy => 1,
        InfluenceClass::Unknown => 0,
    }
}

fn sufficiency_rank(status: SufficiencyStatus) -> u8 {
    match status {
        SufficiencyStatus::Sufficient => 3,
        SufficiencyStatus::Insufficient => 2,
        SufficiencyStatus::Unknown => 1,
        SufficiencyStatus::Failed => 0,
    }
}

fn compare(expectation: &ContextExpectation, context: &CompiledContext) -> Vec<ContextDrift> {
    let mut drifts = Vec::new();

    if expectation.policy_id != context.policy_id {
        drifts.push(ContextDrift::PolicyChanged {
            expected: expectation.policy_id.clone(),
            actual: context.policy_id.clone(),
        });
    }

    for expected in &expectation.nodes {
        match context.node(&expected.node_id) {
            None => drifts.push(ContextDrift::NodeMissing {
                node_id: expected.node_id.clone(),
                kind: expected.kind,
                obligation: expected.obligation.clone(),
            }),
            Some(actual) => {
                if actual.kind != expected.kind {
                    drifts.push(ContextDrift::NodeKindChanged {
                        node_id: expected.node_id.clone(),
                        expected: expected.kind,
                        actual: actual.kind,
                    });
                }
                if actual.obligation != expected.obligation {
                    drifts.push(ContextDrift::NodeObligationChanged {
                        node_id: expected.node_id.clone(),
                        expected: expected.obligation.clone(),
                        actual: actual.obligation.clone(),
                    });
                }
                if expected.locator_required && actual.locator.is_none() {
                    drifts.push(ContextDrift::LocatorLost {
                        node_id: expected.node_id.clone(),
                    });
                }
            }
        }
    }

    if expectation.closed {
        let expected_ids: BTreeSet<&str> = expectation
            .nodes
            .iter()
            .map(|node| node.node_id.as_str())
            .collect();
        for node in &context.nodes {
            if !expected_ids.contains(node.node_id.as_str()) {
                drifts.push(ContextDrift::NodeUnexpected {
                    node_id: node.node_id.clone(),
                    kind: node.kind,
                });
            }
        }
    }

    let covered = context.covered_obligations();
    for obligation in &expectation.required_obligations {
        if !covered.contains(obligation) {
            drifts.push(ContextDrift::ObligationCoverageLost {
                obligation: obligation.clone(),
            });
        }
    }

    let filled = context.filled_invariant_slots();
    for slot in &expectation.required_invariant_slots {
        if !filled.contains(slot) {
            drifts.push(ContextDrift::InvariantSlotUnfilled { slot: slot.clone() });
        }
    }

    for (kind, minimum) in &expectation.minimum_kind_counts {
        let actual = context.count_of_kind(*kind);
        if actual < *minimum {
            drifts.push(ContextDrift::ProtectedKindCountFell {
                kind: *kind,
                expected_at_least: *minimum,
                actual,
            });
        }
    }

    let actual_influence: BTreeMap<String, InfluenceClass> = context
        .omissions
        .groups
        .iter()
        .map(|group| (group.reason.clone(), group.influence))
        .collect();
    for (reason, expected) in &expectation.omission_influence {
        match actual_influence.get(reason) {
            None => drifts.push(ContextDrift::OmissionGroupDisappeared {
                reason: reason.clone(),
            }),
            Some(actual) if actual == expected => {}
            Some(actual) if influence_rank(*actual) < influence_rank(*expected) => {
                drifts.push(ContextDrift::OmissionInfluenceWeakened {
                    reason: reason.clone(),
                    expected: *expected,
                    actual: *actual,
                });
            }
            Some(actual) => drifts.push(ContextDrift::OmissionInfluenceStrengthened {
                reason: reason.clone(),
                expected: *expected,
                actual: *actual,
            }),
        }
    }
    for (reason, influence) in &actual_influence {
        if !expectation.omission_influence.contains_key(reason) {
            drifts.push(ContextDrift::OmissionGroupAppeared {
                reason: reason.clone(),
                influence: *influence,
            });
        }
    }

    if context.sufficiency != expectation.sufficiency {
        let expected = expectation.sufficiency;
        let actual = context.sufficiency;
        if sufficiency_rank(actual) < sufficiency_rank(expected) {
            drifts.push(ContextDrift::SufficiencyWeakened { expected, actual });
        } else {
            drifts.push(ContextDrift::SufficiencyStrengthened { expected, actual });
        }
    }

    let total = context.total_estimate();
    if total.method != expectation.estimator {
        drifts.push(ContextDrift::EstimatorChanged {
            expected: expectation.estimator.label(),
            actual: total.method.label(),
        });
    } else if !expectation.token_band.contains(total.tokens) {
        drifts.push(ContextDrift::TokenTotalOutsideBand {
            band: expectation.token_band,
            actual: total.tokens,
        });
    }

    for (node_id, digest) in &expectation.renderings {
        let actual = context.node(node_id).and_then(|node| node.rendering.as_ref());
        if actual != Some(digest) {
            drifts.push(ContextDrift::RenderingChanged {
                node_id: node_id.clone(),
            });
        }
    }

    drifts
}

/// What kind of case a fixture is.
///
/// 39.21 requires a suite that is not only nominal, and the implementation sequence names the four
/// classes explicitly: "negative, malformed, stale, and adversarial cases".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureClass {
    /// A well-formed input compiling normally.
    Nominal,
    /// Input that violates its own schema, missing units, broken identity.
    Malformed,
    /// Input constructed to defeat the compiler: a rare state that a mean hides, a contradiction
    /// phrased as agreement.
    Adversarial,
    /// Input where evidence genuinely conflicts and both sides must survive.
    Contradiction,
    /// Input where an assay failed, or a measurement is absent rather than negative.
    FailedAssay,
    /// Input whose cached context is past its declared validity, exercising 39.18.
    Stale,
}

impl FixtureClass {
    pub fn as_str(self) -> &'static str {
        match self {
            FixtureClass::Nominal => "nominal",
            FixtureClass::Malformed => "malformed",
            FixtureClass::Adversarial => "adversarial",
            FixtureClass::Contradiction => "contradiction",
            FixtureClass::FailedAssay => "failed_assay",
            FixtureClass::Stale => "stale",
        }
    }
}

/// One fixture with the modality and case class it covers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FixtureEntry {
    pub fixture: ContextFixture,
    /// Table, matrix, image, slide, sequence, literature, timeline — the 39.13 and 39.14 axes.
    pub modality: String,
    pub class: FixtureClass,
}

/// A coverage requirement a bundle does not satisfy.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "gap", rename_all = "snake_case")]
pub enum CoverageGap {
    /// A modality present in the bundle has no fixture of a class 39.21 requires per modality.
    ModalityClassMissing { modality: String, class: FixtureClass },
    /// A class 39.21 requires of the bundle as a whole is absent everywhere.
    BundleClassMissing { class: FixtureClass },
}

impl CoverageGap {
    pub fn describe(&self) -> String {
        match self {
            CoverageGap::ModalityClassMissing { modality, class } => format!(
                "modality `{modality}` has no {} fixture",
                class.as_str()
            ),
            CoverageGap::BundleClassMissing { class } => {
                format!("the bundle has no {} fixture at all", class.as_str())
            }
        }
    }
}

/// The reproducible fixture bundle of 39.21.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FixtureBundle {
    pub bundle_id: String,
    pub entries: Vec<FixtureEntry>,
}

impl FixtureBundle {
    /// Classes every modality must exercise. Invariant 3.
    pub const PER_MODALITY_CLASSES: [FixtureClass; 2] =
        [FixtureClass::Malformed, FixtureClass::Adversarial];

    /// Classes the bundle must exercise somewhere. Invariant 4, plus the stale case the
    /// implementation sequence names, which is what ties this module to 39.18.
    pub const BUNDLE_CLASSES: [FixtureClass; 3] = [
        FixtureClass::Contradiction,
        FixtureClass::FailedAssay,
        FixtureClass::Stale,
    ];

    pub fn new(bundle_id: impl Into<String>) -> Self {
        FixtureBundle {
            bundle_id: bundle_id.into(),
            entries: Vec::new(),
        }
    }

    pub fn with(mut self, fixture: ContextFixture, modality: &str, class: FixtureClass) -> Self {
        self.entries.push(FixtureEntry {
            fixture,
            modality: modality.to_string(),
            class,
        });
        self
    }

    pub fn modalities(&self) -> BTreeSet<String> {
        self.entries
            .iter()
            .map(|entry| entry.modality.clone())
            .collect()
    }

    /// Every requirement of 39.21 that this bundle does not meet, named.
    pub fn coverage_gaps(&self) -> Vec<CoverageGap> {
        let mut gaps = Vec::new();
        let present: BTreeSet<(String, FixtureClass)> = self
            .entries
            .iter()
            .map(|entry| (entry.modality.clone(), entry.class))
            .collect();
        for modality in self.modalities() {
            for class in Self::PER_MODALITY_CLASSES {
                if !present.contains(&(modality.clone(), class)) {
                    gaps.push(CoverageGap::ModalityClassMissing {
                        modality: modality.clone(),
                        class,
                    });
                }
            }
        }
        let classes: BTreeSet<FixtureClass> = self.entries.iter().map(|entry| entry.class).collect();
        for class in Self::BUNDLE_CLASSES {
            if !classes.contains(&class) {
                gaps.push(CoverageGap::BundleClassMissing { class });
            }
        }
        gaps.sort();
        gaps
    }

    /// The gaps as a typed error, reporting the first in deterministic order.
    pub fn validate(&self) -> Result<(), FixtureError> {
        for entry in &self.entries {
            entry.fixture.validate()?;
        }
        if let Some(gap) = self.coverage_gaps().first() {
            return Err(FixtureError::BundleCoverageGap {
                bundle: self.bundle_id.clone(),
                requirement: gap.describe(),
            });
        }
        Ok(())
    }

    /// Content hash over the fixtures' own digests, so a bundle is reproducible without inlining
    /// every expectation into one blob.
    pub fn digest(&self) -> Result<String, FixtureError> {
        let mut rows = Vec::new();
        for entry in &self.entries {
            rows.push(json!({
                "modality": entry.modality,
                "class": entry.class.as_str(),
                "fixture": entry.fixture.digest()?,
            }));
        }
        ContentHash::of_value(&json!({ "bundle": self.bundle_id, "entries": rows }))
            .map(|hash| hash.as_str().to_string())
            .map_err(|error| {
                FixtureError::NotAddressable(self.bundle_id.clone(), error.to_string())
            })
    }
}

/// A convenience for the common shape: a fixture pinned from one reference compile.
pub fn pin_single(
    fixture_id: &str,
    decision_ref: &str,
    context: &CompiledContext,
    tolerance_percent: usize,
) -> Result<ContextFixture, FixtureError> {
    let expectation = ContextFixture::pin(fixture_id, "reference", context, tolerance_percent)?;
    Ok(ContextFixture::new(fixture_id, decision_ref).accepting(expectation))
}

/// The token estimate a fixture band was derived from, for a caller reconstructing a band.
pub fn band_source(context: &CompiledContext) -> TokenEstimate {
    context.total_estimate()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{ContextNode, Visibility};
    use bioprism_section::{OmissionGroup, OmissionManifest};

    fn est(tokens: usize) -> TokenEstimate {
        TokenEstimate::declared(tokens)
    }

    fn manifest(reason: &str, influence: InfluenceClass) -> OmissionManifest {
        let mut manifest = OmissionManifest::default();
        manifest.push(OmissionGroup {
            reason: reason.to_string(),
            influence,
            count: 3,
            bound: None,
            examples: vec![],
        });
        manifest
    }

    fn reference() -> CompiledContext {
        CompiledContext::new(
            "compiler/1.0",
            "decision/glioma-response",
            "molecular",
            "policy/decision-minimal",
            SufficiencyStatus::Sufficient,
        )
        .with_node(
            ContextNode::new("n/build", NodeKind::Invariant, est(6))
                .filling_slot("reference_build")
                .at("world://build"),
        )
        .with_node(
            ContextNode::new("n/idh", NodeKind::Evidence, est(40))
                .serving("o/molecular-subtype")
                .at("world://assay/idh"),
        )
        .with_node(
            ContextNode::new("n/conflict", NodeKind::Contradiction, est(30))
                .serving("o/molecular-subtype")
                .at("world://assay/idh-repeat"),
        )
        .with_node(
            ContextNode::new("n/failed", NodeKind::NegativeEvidence, est(12))
                .serving("o/mgmt")
                .at("world://assay/mgmt-failed"),
        )
        .with_omissions(manifest("duplicate_background", InfluenceClass::Zero))
    }

    fn pinned() -> ContextFixture {
        pin_single("fx/glioma", "decision/glioma-response", &reference(), 10).expect("pins")
    }

    #[test]
    fn a_fixture_pinned_from_a_compile_accepts_that_same_compile() {
        let report = pinned().check(&reference()).expect("checks");
        assert_eq!(report.verdict, FixtureVerdict::Accepted);
        assert!(report.drifts.is_empty());
    }

    #[test]
    fn a_dropped_contradiction_is_reported_as_the_named_node_and_not_as_a_digest_difference() {
        let mut degraded = reference();
        degraded.nodes.retain(|node| node.node_id != "n/conflict");
        let report = pinned().check(&degraded).expect("checks");
        assert_eq!(report.verdict, FixtureVerdict::Rejected);
        assert!(report.drifts.iter().any(|drift| matches!(
            drift,
            ContextDrift::NodeMissing { node_id, kind: NodeKind::Contradiction, .. }
                if node_id == "n/conflict"
        )));
        assert!(report
            .diagnosis()
            .iter()
            .any(|line| line.contains("n/conflict")));
    }

    #[test]
    fn dropping_the_second_of_two_contradictions_is_caught_by_the_protected_kind_count() {
        let with_two = reference().with_node(
            ContextNode::new("n/conflict2", NodeKind::Contradiction, est(10)).at("world://x"),
        );
        let fixture = pin_single("fx/two", "d", &with_two, 50).expect("pins");
        let mut degraded = with_two.clone();
        degraded.nodes.retain(|node| node.node_id != "n/conflict2");
        let report = fixture.check(&degraded).expect("checks");
        assert!(report.drifts.iter().any(|drift| matches!(
            drift,
            ContextDrift::ProtectedKindCountFell {
                kind: NodeKind::Contradiction,
                expected_at_least: 2,
                actual: 1
            }
        )));
    }

    #[test]
    fn a_contradiction_relabelled_as_ordinary_evidence_is_a_critical_drift() {
        let mut relabelled = reference();
        relabelled.nodes[2].kind = NodeKind::Evidence;
        let report = pinned().check(&relabelled).expect("checks");
        let kind_change = report
            .drifts
            .iter()
            .find(|drift| matches!(drift, ContextDrift::NodeKindChanged { .. }))
            .expect("reports the reclassification");
        assert_eq!(kind_change.severity(), DriftSeverity::Critical);
    }

    #[test]
    fn rewording_a_node_never_fails_a_fixture() {
        let with_prose = reference().with_node(
            ContextNode::new("n/note", NodeKind::Evidence, est(5)).rendered_as("first wording"),
        );
        let fixture = pin_single("fx/prose", "d", &with_prose, 20).expect("pins");
        let mut reworded = with_prose.clone();
        let last = reworded.nodes.len() - 1;
        reworded.nodes[last] = reworded.nodes[last].clone().rendered_as("second wording");
        let report = fixture.check(&reworded).expect("checks");
        assert_eq!(report.verdict, FixtureVerdict::AcceptedWithAdvisories);
        assert_eq!(report.advisories().count(), 1);
        assert_eq!(report.critical().count(), 0);
    }

    #[test]
    fn an_omission_group_weakening_from_zero_to_unknown_fails_the_check() {
        let mut weakened = reference();
        weakened.omissions = manifest("duplicate_background", InfluenceClass::Unknown);
        let report = pinned().check(&weakened).expect("checks");
        assert_eq!(report.verdict, FixtureVerdict::Rejected);
        assert!(report.drifts.iter().any(|drift| matches!(
            drift,
            ContextDrift::OmissionInfluenceWeakened {
                expected: InfluenceClass::Zero,
                actual: InfluenceClass::Unknown,
                ..
            }
        )));
    }

    #[test]
    fn an_omission_group_strengthening_is_reported_rather_than_silently_welcomed() {
        let base = reference().with_omissions(manifest("late_arrivals", InfluenceClass::Unknown));
        let fixture = pin_single("fx/strengthen", "d", &base, 10).expect("pins");
        let mut stronger = base.clone();
        stronger.omissions = manifest("late_arrivals", InfluenceClass::Zero);
        let report = fixture.check(&stronger).expect("checks");
        assert!(report.drifts.iter().any(|drift| matches!(
            drift,
            ContextDrift::OmissionInfluenceStrengthened { .. }
        )));
        assert_eq!(report.verdict, FixtureVerdict::Rejected);
    }

    #[test]
    fn a_compiler_that_starts_claiming_sufficiency_does_not_pass_quietly() {
        let base = CompiledContext::new("c", "d", "r", "p", SufficiencyStatus::Unknown)
            .with_node(ContextNode::new("n/a", NodeKind::Evidence, est(20)));
        let fixture = pin_single("fx/suff", "d", &base, 10).expect("pins");
        let mut stronger = base.clone();
        stronger.sufficiency = SufficiencyStatus::Sufficient;
        let report = fixture.check(&stronger).expect("checks");
        assert!(report.drifts.iter().any(|drift| matches!(
            drift,
            ContextDrift::SufficiencyStrengthened {
                expected: SufficiencyStatus::Unknown,
                actual: SufficiencyStatus::Sufficient
            }
        )));
    }

    #[test]
    fn a_changed_estimator_is_critical_and_suppresses_the_meaningless_band_comparison() {
        let mut reestimated = reference();
        for node in &mut reestimated.nodes {
            node.estimate = TokenEstimate::from_provider(node.estimate.tokens * 3, "cl100k");
        }
        let report = pinned().check(&reestimated).expect("checks");
        assert!(report
            .drifts
            .iter()
            .any(|drift| matches!(drift, ContextDrift::EstimatorChanged { .. })));
        assert!(!report
            .drifts
            .iter()
            .any(|drift| matches!(drift, ContextDrift::TokenTotalOutsideBand { .. })));
        assert_eq!(report.verdict, FixtureVerdict::Rejected);
    }

    #[test]
    fn a_token_total_outside_the_band_is_a_regression_and_the_band_is_reported_with_it() {
        let mut bloated = reference();
        bloated.nodes[1].estimate = est(4000);
        let report = pinned().check(&bloated).expect("checks");
        let drift = report
            .drifts
            .iter()
            .find(|drift| matches!(drift, ContextDrift::TokenTotalOutsideBand { .. }))
            .expect("reports the band");
        assert_eq!(drift.severity(), DriftSeverity::Regression);
        assert!(drift.describe().contains("outside the band"));
    }

    #[test]
    fn a_set_valued_golden_accepts_either_of_two_legal_projections() {
        let alternative = CompiledContext::new(
            "compiler/1.0",
            "decision/glioma-response",
            "molecular",
            "policy/decision-minimal",
            SufficiencyStatus::Sufficient,
        )
        .with_node(ContextNode::new("n/build", NodeKind::Invariant, est(6)).filling_slot("reference_build"))
        .with_node(ContextNode::new("n/idh-alt", NodeKind::Evidence, est(38)).serving("o/molecular-subtype"));
        let fixture = ContextFixture::new("fx/set", "decision/glioma-response")
            .accepting(ContextFixture::pin("fx/set", "primary", &reference(), 10).expect("pins"))
            .accepting(
                ContextFixture::pin("fx/set", "alternative", &alternative, 10).expect("pins"),
            );
        assert!(fixture.check(&reference()).expect("checks").verdict.is_accepted());
        let second = fixture.check(&alternative).expect("checks");
        assert!(second.verdict.is_accepted());
        assert_eq!(second.against, "alternative");
    }

    #[test]
    fn when_no_projection_matches_the_report_names_the_closest_one_it_diffed_against() {
        let alternative = CompiledContext::new("compiler/1.0", "d", "molecular", "policy/p", SufficiencyStatus::Unknown)
            .with_node(ContextNode::new("n/a", NodeKind::Evidence, est(10)))
            .with_node(ContextNode::new("n/b", NodeKind::Evidence, est(10)))
            .with_node(ContextNode::new("n/c", NodeKind::Evidence, est(10)));
        let near = CompiledContext::new("compiler/1.0", "d", "molecular", "policy/p", SufficiencyStatus::Unknown)
            .with_node(ContextNode::new("n/a", NodeKind::Evidence, est(10)));
        let fixture = ContextFixture::new("fx/closest", "d")
            .accepting(ContextFixture::pin("fx/closest", "wide", &alternative, 0).expect("pins"))
            .accepting(ContextFixture::pin("fx/closest", "narrow", &near, 0).expect("pins"));
        let mut observed = near.clone();
        observed.nodes[0].estimate = est(11);
        let report = fixture.check(&observed).expect("checks");
        assert_eq!(report.against, "narrow");
        assert_eq!(report.all.len(), 2);
        assert!(report.all.iter().any(|comparison| comparison.label == "wide"));
    }

    #[test]
    fn the_closest_projection_is_the_same_one_on_every_run() {
        let fixture = pinned();
        let mut degraded = reference();
        degraded.nodes.remove(0);
        let first = fixture.check(&degraded).expect("checks");
        for _ in 0..8 {
            assert_eq!(fixture.check(&degraded).expect("checks"), first);
        }
    }

    #[test]
    fn pinning_a_compile_that_contains_evaluator_holdout_state_is_refused() {
        let leaky = reference().with_node(
            ContextNode::new("n/answer", NodeKind::Evidence, est(4))
                .with_visibility(Visibility::Holdout),
        );
        assert!(matches!(
            ContextFixture::pin("fx/leak", "reference", &leaky, 10),
            Err(FixtureError::HoldoutLeakedIntoFixture { node, .. }) if node == "n/answer"
        ));
    }

    #[test]
    fn a_hand_written_golden_that_names_a_holdout_node_fails_validation() {
        let mut fixture = pinned();
        fixture.holdout_nodes.insert("n/idh".to_string());
        assert!(matches!(
            fixture.validate(),
            Err(FixtureError::HoldoutLeakedIntoFixture { .. })
        ));
    }

    #[test]
    fn a_golden_with_no_accepted_projections_is_refused_rather_than_passing_everything() {
        let empty = ContextFixture::new("fx/empty", "d");
        assert!(matches!(
            empty.validate(),
            Err(FixtureError::NoAcceptedProjections(_))
        ));
        assert!(empty.check(&reference()).is_err());
    }

    #[test]
    fn a_golden_that_pins_wording_for_a_node_it_does_not_otherwise_expect_is_refused() {
        let mut fixture = pinned();
        fixture.accepted[0]
            .renderings
            .insert("n/nowhere".to_string(), "0".repeat(64));
        assert!(matches!(
            fixture.validate(),
            Err(FixtureError::RenderingPinnedWithoutSemantics { .. })
        ));
    }

    #[test]
    fn a_closed_expectation_reports_an_added_node_and_an_open_one_does_not() {
        let mut open = pinned();
        let mut closed = pinned();
        closed.accepted[0].closed = true;
        open.accepted[0].closed = false;
        let grown = reference().with_node(ContextNode::new("n/extra", NodeKind::Evidence, est(2)));
        assert!(!closed
            .check(&grown)
            .expect("checks")
            .drifts
            .iter()
            .all(|drift| !matches!(drift, ContextDrift::NodeUnexpected { .. })));
        assert!(open
            .check(&grown)
            .expect("checks")
            .drifts
            .iter()
            .all(|drift| !matches!(drift, ContextDrift::NodeUnexpected { .. })));
    }

    #[test]
    fn losing_a_source_locator_is_reported_because_the_node_can_no_longer_be_expanded() {
        let mut stripped = reference();
        stripped.nodes[1].locator = None;
        let report = pinned().check(&stripped).expect("checks");
        assert!(report
            .drifts
            .iter()
            .any(|drift| matches!(drift, ContextDrift::LocatorLost { node_id } if node_id == "n/idh")));
    }

    #[test]
    fn a_bundle_missing_an_adversarial_case_for_a_modality_names_the_modality_and_the_class() {
        let bundle = FixtureBundle::new("bundle/1")
            .with(pinned(), "matrix", FixtureClass::Nominal)
            .with(pinned(), "matrix", FixtureClass::Malformed)
            .with(pinned(), "matrix", FixtureClass::Contradiction)
            .with(pinned(), "matrix", FixtureClass::FailedAssay)
            .with(pinned(), "matrix", FixtureClass::Stale);
        let gaps = bundle.coverage_gaps();
        assert_eq!(
            gaps,
            vec![CoverageGap::ModalityClassMissing {
                modality: "matrix".to_string(),
                class: FixtureClass::Adversarial
            }]
        );
        assert!(matches!(
            bundle.validate(),
            Err(FixtureError::BundleCoverageGap { .. })
        ));
    }

    #[test]
    fn a_bundle_with_no_failed_assay_fixture_anywhere_is_incomplete() {
        let bundle = FixtureBundle::new("bundle/2")
            .with(pinned(), "table", FixtureClass::Malformed)
            .with(pinned(), "table", FixtureClass::Adversarial)
            .with(pinned(), "table", FixtureClass::Contradiction)
            .with(pinned(), "table", FixtureClass::Stale);
        assert!(bundle
            .coverage_gaps()
            .contains(&CoverageGap::BundleClassMissing {
                class: FixtureClass::FailedAssay
            }));
    }

    #[test]
    fn a_complete_bundle_validates_and_is_content_addressable() {
        let bundle = FixtureBundle::new("bundle/3")
            .with(pinned(), "table", FixtureClass::Malformed)
            .with(pinned(), "table", FixtureClass::Adversarial)
            .with(pinned(), "table", FixtureClass::Contradiction)
            .with(pinned(), "table", FixtureClass::FailedAssay)
            .with(pinned(), "table", FixtureClass::Stale);
        assert!(bundle.validate().is_ok());
        assert_eq!(bundle.digest().expect("digests").len(), 64);
        assert_eq!(bundle.digest().expect("digests"), bundle.digest().expect("digests"));
    }

    #[test]
    fn the_diagnosis_lists_defects_before_wording() {
        let mut broken = reference();
        broken.nodes.retain(|node| node.node_id != "n/failed");
        broken.nodes.push(
            ContextNode::new("n/note", NodeKind::Evidence, est(1)).rendered_as("changed"),
        );
        let mut fixture = pinned();
        fixture.accepted[0]
            .nodes
            .push(NodeExpectation {
                node_id: "n/note".to_string(),
                kind: NodeKind::Evidence,
                obligation: None,
                invariant_slot: None,
                locator_required: false,
            });
        fixture.accepted[0]
            .renderings
            .insert("n/note".to_string(), "a".repeat(64));
        let report = fixture.check(&broken).expect("checks");
        let lines = report.diagnosis();
        let first_advisory = lines
            .iter()
            .position(|line| line.contains("wording"))
            .expect("wording drift is reported");
        assert!(first_advisory > 0, "defects must sort before wording");
    }

    #[test]
    fn band_source_reports_the_estimate_the_band_came_from_with_its_method_attached() {
        let estimate = band_source(&reference());
        assert!(!estimate.method.is_measured());
        assert_eq!(estimate.tokens, 6 + 40 + 30 + 12);
    }

    #[test]
    fn a_fixture_report_survives_a_json_round_trip() {
        let report = pinned().check(&reference()).expect("checks");
        let text = serde_json::to_string(&report).expect("serialises");
        let back: FixtureReport = serde_json::from_str(&text).expect("deserialises");
        assert_eq!(back, report);
    }
}
