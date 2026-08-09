//! Ablations and experimental design (39.23).
//!
//! 39.23 wants to know whether graph compilation, mandatory closure, value-based expansion, delta
//! context and role projection each contribute. Its first invariant is the one everything else
//! hangs off: *"hold world state and inference architecture fixed except for declared component"*.
//!
//! # The refusal is reused, not restated
//!
//! `bioprism-evalengine` already refuses to attribute a difference when more than one component
//! varied between two matched-fork arms — the honest statement is "something in {a, b} did it",
//! which is the end-to-end comparison the fork existed to replace. This module applies that same
//! rule one step earlier, to the **design**, so a confounded ablation is refused before it is run
//! rather than after it has produced a number somebody wants to believe. The division is
//! deliberate: `evalengine` refuses an *attribution*, this refuses a *contrast*, and neither
//! reimplements the other's scoring.
//!
//! [`ContrastVerdict::Refused`] carries a [`ConfoundReason`] naming the varied set, so a caller can
//! go and run the two contrasts that would have answered the question.
//!
//! # Cost is never a result on its own
//!
//! 39.22 ends with "compression ratio alone is never a release criterion", and 39.23's third
//! invariant says report cost and validity jointly. [`AblationDesign::report`] therefore returns
//! [`AblationError::CostReportedWithoutValidity`] when an arm has a token estimate and no scored
//! outcome. A token saving with no validity number beside it is the exact artifact the section
//! exists to prevent being persuasive.
//!
//! # Pseudo-replication
//!
//! 39.23's second invariant clusters statistics by parent world or cell, and its failure list names
//! "pseudo-replication from mutations". Two arms derived from the same parent cell are one
//! observation wearing two hats, so a contrast across a cluster boundary is refused
//! ([`ConfoundReason::ClusterMismatch`]) and [`AblationDesign::independent_clusters`] reports how
//! many genuinely independent units a design has.
//!
//! # The one clause of 39.25 that is a runtime invariant
//!
//! 39.25 is a delivery plan and is classified as prose in this crate's documentation. One of its
//! four commitments is not about scheduling: *"full-context baseline remains available"*. Without a
//! full-context reference arm a token saving has nothing to be a saving against, so
//! [`AblationDesign::validate`] requires one. That is the only place 39.25 is enforced.
//!
//! # Not implemented
//!
//! No statistics. There is no effect estimator, no interaction analysis, no confidence interval and
//! no randomisation. 39.23's "component effect estimates" require a sample this crate never sees,
//! and inventing one from two arms would be worse than the refusals above are good. What is here is
//! the design validity layer: which comparisons are licensed, and which are not.

use crate::context::estimates_are_comparable;
use crate::error::AblationError;
use bioprism_obligation::TokenEstimate;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The components 39.23 names, plus an escape hatch for anything else a design declares.
///
/// Used to build canonical setting keys. Settings are stored as string-keyed maps rather than
/// enum-keyed ones so a design can declare a component this crate has never heard of without the
/// vocabulary here becoming a ceiling on what may be ablated.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "component", rename_all = "snake_case")]
pub enum ContextComponent {
    GraphCompilation,
    MandatoryClosure,
    ValueBasedExpansion,
    DeltaContext,
    RoleProjection,
    LearnedSelector,
    TokenBudget,
    Other { name: String },
}

impl ContextComponent {
    pub fn key(&self) -> String {
        match self {
            ContextComponent::GraphCompilation => "graph_compilation".to_string(),
            ContextComponent::MandatoryClosure => "mandatory_closure".to_string(),
            ContextComponent::ValueBasedExpansion => "value_based_expansion".to_string(),
            ContextComponent::DeltaContext => "delta_context".to_string(),
            ContextComponent::RoleProjection => "role_projection".to_string(),
            ContextComponent::LearnedSelector => "learned_selector".to_string(),
            ContextComponent::TokenBudget => "token_budget".to_string(),
            ContextComponent::Other { name } => name.clone(),
        }
    }
}

/// One component's setting on one arm, for reporting.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ComponentSetting {
    pub component: String,
    pub value: String,
}

/// A learned selector and the holdout it was gated against.
///
/// 39.23's fourth invariant requires hidden holdouts for learned selectors. The holdout is
/// `Option` rather than required so that an arm which forgot one is *representable* — and therefore
/// refusable at validation — instead of impossible to write down and therefore never checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LearnedSelector {
    pub selector_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden_holdout: Option<String>,
}

/// One arm of an ablation: a full declaration of the settings in force.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AblationArm {
    pub arm_id: String,
    /// Every declared component setting. A component absent here was *not declared*, which is
    /// different from being absent — see [`ConfoundReason::DeclaredOnOneSideOnly`].
    pub settings: BTreeMap<String, String>,
    /// The parent world or decision cell this arm was forked from. Two arms in different clusters
    /// are not a matched pair.
    pub cluster: String,
    /// Whether this arm runs the uncompiled full context. 39.25 requires one to exist.
    #[serde(default)]
    pub full_context_reference: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub learned_selector: Option<LearnedSelector>,
}

impl AblationArm {
    pub fn new(arm_id: impl Into<String>, cluster: impl Into<String>) -> Self {
        AblationArm {
            arm_id: arm_id.into(),
            settings: BTreeMap::new(),
            cluster: cluster.into(),
            full_context_reference: false,
            learned_selector: None,
        }
    }

    pub fn set(mut self, component: ContextComponent, value: impl Into<String>) -> Self {
        self.settings.insert(component.key(), value.into());
        self
    }

    pub fn as_full_context_reference(mut self) -> Self {
        self.full_context_reference = true;
        self
    }

    pub fn with_learned_selector(mut self, selector: LearnedSelector) -> Self {
        self.learned_selector = Some(selector);
        self
    }
}

/// What a design committed to before seeing any result.
///
/// 39.23's inputs include "predeclared outcomes and margins". A margin chosen after the fact is not
/// a margin, and a design without this is refused rather than scored against whatever threshold
/// makes the result look best.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreDeclaration {
    pub outcomes: Vec<String>,
    /// The noninferiority margin on the primary validity outcome, in the outcome's own units.
    pub noninferiority_margin: f64,
    /// Whether the analysis was registered before the runs. Recorded rather than assumed.
    #[serde(default)]
    pub registered: bool,
}

impl PreDeclaration {
    pub fn new(outcomes: Vec<String>, noninferiority_margin: f64) -> Self {
        PreDeclaration {
            outcomes,
            noninferiority_margin,
            registered: false,
        }
    }

    pub fn registered(mut self) -> Self {
        self.registered = true;
        self
    }
}

/// A validity outcome somebody else scored.
///
/// This crate never produces one. The field [`ValidityOutcome::scored_by`] exists so a report
/// carries whose judgement it is, and so an outcome scored by a judge that saw the policy label —
/// a failure mode 39.23 names explicitly — is at least attributable when it is discovered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidityOutcome {
    pub valid: usize,
    pub total: usize,
    pub scored_by: String,
    /// Whether the scorer was blind to which arm it was scoring.
    #[serde(default)]
    pub blinded: bool,
}

impl ValidityOutcome {
    pub fn new(valid: usize, total: usize, scored_by: impl Into<String>) -> Self {
        ValidityOutcome {
            valid,
            total,
            scored_by: scored_by.into(),
            blinded: false,
        }
    }

    pub fn blinded(mut self) -> Self {
        self.blinded = true;
        self
    }
}

/// What one arm produced: cost and validity, together or not at all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArmOutcome {
    pub arm_id: String,
    /// Estimated token cost. Never a measurement; carries its estimator.
    pub cost: TokenEstimate,
    /// The scored validity, when one exists. `None` is representable so that
    /// [`AblationError::CostReportedWithoutValidity`] can fire instead of a cost-only report
    /// quietly becoming the result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validity: Option<ValidityOutcome>,
}

impl ArmOutcome {
    pub fn new(arm_id: impl Into<String>, cost: TokenEstimate) -> Self {
        ArmOutcome {
            arm_id: arm_id.into(),
            cost,
            validity: None,
        }
    }

    pub fn scored(mut self, validity: ValidityOutcome) -> Self {
        self.validity = Some(validity);
        self
    }
}

/// A declared comparison between two arms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contrast {
    pub contrast_id: String,
    pub baseline: String,
    pub variant: String,
    /// Components the design declares it is holding fixed. Checked against the arms, not trusted.
    #[serde(default)]
    pub held_fixed: BTreeSet<String>,
    /// Whether the intervention was randomised or otherwise controlled. Only a controlled contrast
    /// licenses a causal reading.
    #[serde(default)]
    pub controlled: bool,
}

impl Contrast {
    pub fn new(
        contrast_id: impl Into<String>,
        baseline: impl Into<String>,
        variant: impl Into<String>,
    ) -> Self {
        Contrast {
            contrast_id: contrast_id.into(),
            baseline: baseline.into(),
            variant: variant.into(),
            held_fixed: BTreeSet::new(),
            controlled: false,
        }
    }

    pub fn holding_fixed<I, S>(mut self, components: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.held_fixed
            .extend(components.into_iter().map(Into::into));
        self
    }

    pub fn controlled(mut self) -> Self {
        self.controlled = true;
        self
    }
}

/// Why a contrast cannot attribute a difference to anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "confound", rename_all = "snake_case")]
pub enum ConfoundReason {
    /// The arms differ in more than one component. Every honest statement is about the set.
    MoreThanOneComponentVaried { varied: BTreeSet<String> },
    /// The arms are identical. A difference in result here is run-to-run instability, and naming a
    /// component for it would attribute noise.
    NothingVaried,
    /// A component the design declared it was holding fixed actually moved.
    HeldFixedComponentMoved {
        component: String,
        baseline: String,
        variant: String,
    },
    /// One arm declares a component the other does not mention. "Varied" is undefined: absence of a
    /// declaration is not a value.
    DeclaredOnOneSideOnly { component: String },
    /// The arms come from different parent worlds or cells, so they are not a matched pair.
    ClusterMismatch {
        baseline_cluster: String,
        variant_cluster: String,
    },
    /// A learned selector was in play with no hidden holdout declared. 39.23's fourth invariant.
    LearnedSelectorWithoutHiddenHoldout { arm: String, selector: String },
}

impl ConfoundReason {
    pub fn describe(&self) -> String {
        match self {
            ConfoundReason::MoreThanOneComponentVaried { varied } => format!(
                "{} components varied ({}); run one contrast per component instead",
                varied.len(),
                varied.iter().cloned().collect::<Vec<_>>().join(", ")
            ),
            ConfoundReason::NothingVaried => {
                "the arms are identical, so any difference is instability rather than an effect"
                    .to_string()
            }
            ConfoundReason::HeldFixedComponentMoved {
                component,
                baseline,
                variant,
            } => format!(
                "`{component}` was declared held fixed but moved from `{baseline}` to `{variant}`"
            ),
            ConfoundReason::DeclaredOnOneSideOnly { component } => format!(
                "`{component}` is declared on one arm only, so whether it varied is undefined"
            ),
            ConfoundReason::ClusterMismatch {
                baseline_cluster,
                variant_cluster,
            } => format!(
                "arms come from different parents (`{baseline_cluster}` and `{variant_cluster}`), \
                 so they are not a matched pair"
            ),
            ConfoundReason::LearnedSelectorWithoutHiddenHoldout { arm, selector } => format!(
                "arm `{arm}` runs learned selector `{selector}` with no hidden holdout declared"
            ),
        }
    }
}

/// How strongly a licensed contrast may be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributionClaim {
    /// Randomised or otherwise controlled: the component caused the difference under the evaluated
    /// distribution.
    Causal,
    /// Not controlled: the component is associated with the difference and nothing stronger was
    /// established. Carried with the verdict rather than left to a footnote.
    Descriptive,
}

/// Whether a contrast licenses an attribution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum ContrastVerdict {
    /// Exactly one declared component varied, everything held fixed held, and both arms share a
    /// parent. A difference may be attributed to `component`, at the stated claim strength.
    Attributable {
        component: String,
        claim: AttributionClaim,
    },
    /// The contrast attributes nothing, and the reason names what would have to change.
    Refused { reason: ConfoundReason },
}

impl ContrastVerdict {
    pub fn is_attributable(&self) -> bool {
        matches!(self, ContrastVerdict::Attributable { .. })
    }

    pub fn component(&self) -> Option<&str> {
        match self {
            ContrastVerdict::Attributable { component, .. } => Some(component),
            ContrastVerdict::Refused { .. } => None,
        }
    }
}

/// A full ablation design: arms, the contrasts drawn between them, and the pre-declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AblationDesign {
    pub design_id: String,
    pub arms: Vec<AblationArm>,
    pub contrasts: Vec<Contrast>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predeclaration: Option<PreDeclaration>,
}

impl AblationDesign {
    pub fn new(design_id: impl Into<String>) -> Self {
        AblationDesign {
            design_id: design_id.into(),
            arms: Vec::new(),
            contrasts: Vec::new(),
            predeclaration: None,
        }
    }

    pub fn with_arm(mut self, arm: AblationArm) -> Self {
        self.arms.push(arm);
        self
    }

    pub fn with_contrast(mut self, contrast: Contrast) -> Self {
        self.contrasts.push(contrast);
        self
    }

    pub fn predeclaring(mut self, predeclaration: PreDeclaration) -> Self {
        self.predeclaration = Some(predeclaration);
        self
    }

    pub fn arm(&self, arm_id: &str) -> Option<&AblationArm> {
        self.arms.iter().find(|arm| arm.arm_id == arm_id)
    }

    /// Distinct parent worlds or cells across the arms.
    ///
    /// The denominator for anything that wants to call itself independent. A design whose arms all
    /// descend from one mutated parent has one cluster however many arms it declares.
    pub fn independent_clusters(&self) -> BTreeSet<String> {
        self.arms.iter().map(|arm| arm.cluster.clone()).collect()
    }

    /// Structural checks on the design, before any run.
    pub fn validate(&self) -> Result<(), AblationError> {
        if self.arms.len() < 2 {
            return Err(AblationError::TooFewArms(self.design_id.clone()));
        }
        let mut seen = BTreeSet::new();
        for arm in &self.arms {
            if !seen.insert(arm.arm_id.clone()) {
                return Err(AblationError::DuplicateArm {
                    design: self.design_id.clone(),
                    arm: arm.arm_id.clone(),
                });
            }
        }
        if !self.arms.iter().any(|arm| arm.full_context_reference) {
            return Err(AblationError::NoFullContextBaseline {
                design: self.design_id.clone(),
            });
        }
        if self.predeclaration.is_none() {
            return Err(AblationError::NoPreDeclaration {
                design: self.design_id.clone(),
            });
        }
        for contrast in &self.contrasts {
            for arm_id in [&contrast.baseline, &contrast.variant] {
                if self.arm(arm_id).is_none() {
                    return Err(AblationError::UnknownArm {
                        contrast: contrast.contrast_id.clone(),
                        arm: arm_id.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Judge one contrast. Refuses rather than attributing whenever the comparison is not clean.
    ///
    /// The checks run in a fixed order — cluster, holdout, declaration symmetry, held-fixed
    /// violation, varied count — so a contrast with two problems always reports the same one, and a
    /// caller fixing them works through a stable list.
    pub fn judge(&self, contrast: &Contrast) -> Result<ContrastVerdict, AblationError> {
        let baseline = self
            .arm(&contrast.baseline)
            .ok_or_else(|| AblationError::UnknownArm {
                contrast: contrast.contrast_id.clone(),
                arm: contrast.baseline.clone(),
            })?;
        let variant = self
            .arm(&contrast.variant)
            .ok_or_else(|| AblationError::UnknownArm {
                contrast: contrast.contrast_id.clone(),
                arm: contrast.variant.clone(),
            })?;

        if baseline.cluster != variant.cluster {
            return Ok(refused(ConfoundReason::ClusterMismatch {
                baseline_cluster: baseline.cluster.clone(),
                variant_cluster: variant.cluster.clone(),
            }));
        }

        for arm in [baseline, variant] {
            if let Some(selector) = &arm.learned_selector {
                if selector.hidden_holdout.is_none() {
                    return Ok(refused(
                        ConfoundReason::LearnedSelectorWithoutHiddenHoldout {
                            arm: arm.arm_id.clone(),
                            selector: selector.selector_id.clone(),
                        },
                    ));
                }
            }
        }

        let left: BTreeSet<&String> = baseline.settings.keys().collect();
        let right: BTreeSet<&String> = variant.settings.keys().collect();
        if let Some(component) = left.symmetric_difference(&right).min() {
            return Ok(refused(ConfoundReason::DeclaredOnOneSideOnly {
                component: (*component).clone(),
            }));
        }

        let varied: BTreeSet<String> = baseline
            .settings
            .iter()
            .filter(|(key, value)| {
                variant
                    .settings
                    .get(*key)
                    .is_some_and(|other| other != *value)
            })
            .map(|(key, _)| key.clone())
            .collect();

        if let Some(component) = varied.intersection(&contrast.held_fixed).min() {
            return Ok(refused(ConfoundReason::HeldFixedComponentMoved {
                component: component.clone(),
                baseline: baseline.settings[component].clone(),
                variant: variant.settings[component].clone(),
            }));
        }

        match varied.len() {
            0 => Ok(refused(ConfoundReason::NothingVaried)),
            1 => Ok(ContrastVerdict::Attributable {
                component: varied.into_iter().next().expect("exactly one"),
                claim: if contrast.controlled {
                    AttributionClaim::Causal
                } else {
                    AttributionClaim::Descriptive
                },
            }),
            _ => Ok(refused(ConfoundReason::MoreThanOneComponentVaried {
                varied,
            })),
        }
    }

    /// Judge every contrast and pair each with the arms' joint cost and validity.
    ///
    /// Refuses when any arm reports a cost with no scored validity beside it. This is the only
    /// place in the crate where a missing input is an error rather than a third state, and it is
    /// deliberate: an unscored arm in a report *is* a cost-only claim, whatever the caller intended.
    pub fn report(
        &self,
        outcomes: &BTreeMap<String, ArmOutcome>,
    ) -> Result<AblationReport, AblationError> {
        self.validate()?;
        for contrast in &self.contrasts {
            for arm_id in [&contrast.baseline, &contrast.variant] {
                if let Some(outcome) = outcomes.get(arm_id) {
                    if outcome.validity.is_none() {
                        return Err(AblationError::CostReportedWithoutValidity {
                            design: self.design_id.clone(),
                            contrast: contrast.contrast_id.clone(),
                        });
                    }
                }
            }
        }
        let mut verdicts = Vec::new();
        for contrast in &self.contrasts {
            verdicts.push(ContrastReport {
                contrast_id: contrast.contrast_id.clone(),
                verdict: self.judge(contrast)?,
                baseline: outcomes.get(&contrast.baseline).cloned(),
                variant: outcomes.get(&contrast.variant).cloned(),
            });
        }
        Ok(AblationReport {
            design_id: self.design_id.clone(),
            contrasts: verdicts,
            independent_clusters: self.independent_clusters(),
        })
    }
}

fn refused(reason: ConfoundReason) -> ContrastVerdict {
    ContrastVerdict::Refused { reason }
}

/// One contrast's verdict with the outcomes it was drawn between.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContrastReport {
    pub contrast_id: String,
    pub verdict: ContrastVerdict,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline: Option<ArmOutcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<ArmOutcome>,
}

impl ContrastReport {
    /// The token difference, when both arms were estimated by the same rule.
    ///
    /// `None` when the two costs are not comparable per
    /// [`crate::context::estimates_are_comparable`], because a difference between two rulers is not
    /// a saving. The rule `bioprism-docgraph` applies to sums, applied to subtraction.
    pub fn token_difference(&self) -> Option<i64> {
        let (baseline, variant) = (self.baseline.as_ref()?, self.variant.as_ref()?);
        if !estimates_are_comparable(&baseline.cost, &variant.cost) {
            return None;
        }
        Some(variant.cost.tokens as i64 - baseline.cost.tokens as i64)
    }
}

/// The result of judging a whole design.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AblationReport {
    pub design_id: String,
    pub contrasts: Vec<ContrastReport>,
    pub independent_clusters: BTreeSet<String>,
}

impl AblationReport {
    pub fn attributable(&self) -> impl Iterator<Item = &ContrastReport> {
        self.contrasts
            .iter()
            .filter(|report| report.verdict.is_attributable())
    }

    pub fn refusals(&self) -> impl Iterator<Item = &ContrastReport> {
        self.contrasts
            .iter()
            .filter(|report| !report.verdict.is_attributable())
    }

    /// One line per refused contrast, saying what would have to change.
    pub fn explain_refusals(&self) -> Vec<String> {
        let mut lines: Vec<String> = self
            .refusals()
            .filter_map(|report| match &report.verdict {
                ContrastVerdict::Refused { reason } => Some(format!(
                    "contrast `{}` attributes nothing: {}",
                    report.contrast_id,
                    reason.describe()
                )),
                ContrastVerdict::Attributable { .. } => None,
            })
            .collect();
        lines.sort();
        lines
    }

    /// Whether the design has more than one genuinely independent parent.
    ///
    /// A single cluster is not an error — one cell deeply ablated is a legitimate study — but it
    /// bounds what may be said, so the report states it rather than leaving it to be inferred from
    /// the arm count.
    pub fn has_replication(&self) -> bool {
        self.independent_clusters.len() > 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn est(tokens: usize) -> TokenEstimate {
        TokenEstimate::declared(tokens)
    }

    fn base_arm(id: &str) -> AblationArm {
        AblationArm::new(id, "cell/1")
            .set(ContextComponent::GraphCompilation, "on")
            .set(ContextComponent::MandatoryClosure, "on")
            .set(ContextComponent::ValueBasedExpansion, "greedy")
            .set(ContextComponent::DeltaContext, "off")
            .set(ContextComponent::RoleProjection, "off")
    }

    fn design() -> AblationDesign {
        AblationDesign::new("ablation/context-components")
            .with_arm(base_arm("full").as_full_context_reference())
            .with_arm(base_arm("baseline"))
            .with_arm(base_arm("no-closure").set(ContextComponent::MandatoryClosure, "off"))
            .predeclaring(
                PreDeclaration::new(vec!["decision_validity".to_string()], 0.02).registered(),
            )
    }

    #[test]
    fn a_contrast_varying_exactly_one_component_attributes_to_that_component() {
        let verdict = design()
            .judge(&Contrast::new("c/closure", "baseline", "no-closure"))
            .expect("judges");
        assert_eq!(verdict.component(), Some("mandatory_closure"));
    }

    #[test]
    fn a_contrast_varying_two_components_attributes_nothing_and_names_both() {
        let confounded = design().with_arm(
            base_arm("no-closure-and-delta")
                .set(ContextComponent::MandatoryClosure, "off")
                .set(ContextComponent::DeltaContext, "on"),
        );
        let verdict = confounded
            .judge(&Contrast::new("c/both", "baseline", "no-closure-and-delta"))
            .expect("judges");
        let ContrastVerdict::Refused {
            reason: ConfoundReason::MoreThanOneComponentVaried { varied },
        } = &verdict
        else {
            panic!("expected a confound refusal, got {verdict:?}");
        };
        assert!(varied.contains("mandatory_closure"));
        assert!(varied.contains("delta_context"));
        assert!(!verdict.is_attributable());
    }

    #[test]
    fn two_identical_arms_reaching_different_results_are_instability_not_an_effect() {
        let same = design().with_arm(base_arm("twin"));
        let verdict = same
            .judge(&Contrast::new("c/twin", "baseline", "twin"))
            .expect("judges");
        assert!(matches!(
            verdict,
            ContrastVerdict::Refused {
                reason: ConfoundReason::NothingVaried
            }
        ));
    }

    #[test]
    fn a_component_declared_on_one_arm_only_makes_varied_undefined_rather_than_true() {
        let asymmetric =
            design().with_arm(base_arm("extra").set(ContextComponent::TokenBudget, "8000"));
        let verdict = asymmetric
            .judge(&Contrast::new("c/asym", "baseline", "extra"))
            .expect("judges");
        assert!(matches!(
            verdict,
            ContrastVerdict::Refused {
                reason: ConfoundReason::DeclaredOnOneSideOnly { ref component }
            } if component == "token_budget"
        ));
    }

    #[test]
    fn a_declared_control_that_actually_moved_breaks_the_match_before_the_varied_count_is_reached() {
        let contrast = Contrast::new("c/closure", "baseline", "no-closure")
            .holding_fixed(["mandatory_closure"]);
        let verdict = design().judge(&contrast).expect("judges");
        assert!(matches!(
            verdict,
            ContrastVerdict::Refused {
                reason: ConfoundReason::HeldFixedComponentMoved { ref component, .. }
            } if component == "mandatory_closure"
        ));
    }

    #[test]
    fn arms_from_different_parent_cells_are_not_a_matched_pair() {
        let split = design().with_arm(
            AblationArm::new("other-cell", "cell/2")
                .set(ContextComponent::GraphCompilation, "on")
                .set(ContextComponent::MandatoryClosure, "off")
                .set(ContextComponent::ValueBasedExpansion, "greedy")
                .set(ContextComponent::DeltaContext, "off")
                .set(ContextComponent::RoleProjection, "off"),
        );
        let verdict = split
            .judge(&Contrast::new("c/cross", "baseline", "other-cell"))
            .expect("judges");
        assert!(matches!(
            verdict,
            ContrastVerdict::Refused {
                reason: ConfoundReason::ClusterMismatch { .. }
            }
        ));
    }

    #[test]
    fn a_learned_selector_with_no_hidden_holdout_is_refused() {
        let learned = design().with_arm(base_arm("learned").with_learned_selector(
            LearnedSelector {
                selector_id: "ranker/v2".to_string(),
                hidden_holdout: None,
            },
        ));
        let verdict = learned
            .judge(&Contrast::new("c/learned", "baseline", "learned"))
            .expect("judges");
        assert!(matches!(
            verdict,
            ContrastVerdict::Refused {
                reason: ConfoundReason::LearnedSelectorWithoutHiddenHoldout { .. }
            }
        ));
    }

    #[test]
    fn a_learned_selector_gated_on_a_declared_holdout_is_admitted() {
        let learned = design().with_arm(
            base_arm("learned")
                .set(ContextComponent::ValueBasedExpansion, "learned")
                .with_learned_selector(LearnedSelector {
                    selector_id: "ranker/v2".to_string(),
                    hidden_holdout: Some("holdout/2026-q1".to_string()),
                }),
        );
        let verdict = learned
            .judge(&Contrast::new("c/learned", "baseline", "learned"))
            .expect("judges");
        assert_eq!(verdict.component(), Some("value_based_expansion"));
    }

    #[test]
    fn an_uncontrolled_contrast_yields_a_descriptive_claim_and_a_controlled_one_a_causal_claim() {
        let uncontrolled = design()
            .judge(&Contrast::new("c/a", "baseline", "no-closure"))
            .expect("judges");
        let controlled = design()
            .judge(&Contrast::new("c/b", "baseline", "no-closure").controlled())
            .expect("judges");
        assert!(matches!(
            uncontrolled,
            ContrastVerdict::Attributable {
                claim: AttributionClaim::Descriptive,
                ..
            }
        ));
        assert!(matches!(
            controlled,
            ContrastVerdict::Attributable {
                claim: AttributionClaim::Causal,
                ..
            }
        ));
    }

    #[test]
    fn a_design_with_no_full_context_reference_arm_is_refused() {
        let no_reference = AblationDesign::new("d")
            .with_arm(base_arm("a"))
            .with_arm(base_arm("b").set(ContextComponent::DeltaContext, "on"))
            .predeclaring(PreDeclaration::new(vec!["v".to_string()], 0.01));
        assert!(matches!(
            no_reference.validate(),
            Err(AblationError::NoFullContextBaseline { .. })
        ));
    }

    #[test]
    fn a_design_with_no_predeclared_margin_is_refused() {
        let unregistered = AblationDesign::new("d")
            .with_arm(base_arm("a").as_full_context_reference())
            .with_arm(base_arm("b").set(ContextComponent::DeltaContext, "on"));
        assert!(matches!(
            unregistered.validate(),
            Err(AblationError::NoPreDeclaration { .. })
        ));
    }

    #[test]
    fn a_contrast_naming_an_arm_the_design_does_not_declare_is_refused() {
        let design = design().with_contrast(Contrast::new("c/ghost", "baseline", "nowhere"));
        assert!(matches!(
            design.validate(),
            Err(AblationError::UnknownArm { .. })
        ));
    }

    #[test]
    fn a_cost_reported_with_no_validity_outcome_is_refused_rather_than_becoming_the_result() {
        let design = design().with_contrast(Contrast::new("c/closure", "baseline", "no-closure"));
        let mut outcomes = BTreeMap::new();
        outcomes.insert(
            "baseline".to_string(),
            ArmOutcome::new("baseline", est(1200))
                .scored(ValidityOutcome::new(48, 50, "oracle/deterministic")),
        );
        outcomes.insert(
            "no-closure".to_string(),
            ArmOutcome::new("no-closure", est(400)),
        );
        assert!(matches!(
            design.report(&outcomes),
            Err(AblationError::CostReportedWithoutValidity { .. })
        ));
    }

    fn scored_outcomes() -> BTreeMap<String, ArmOutcome> {
        let mut outcomes = BTreeMap::new();
        outcomes.insert(
            "baseline".to_string(),
            ArmOutcome::new("baseline", est(1200))
                .scored(ValidityOutcome::new(48, 50, "oracle/deterministic").blinded()),
        );
        outcomes.insert(
            "no-closure".to_string(),
            ArmOutcome::new("no-closure", est(400))
                .scored(ValidityOutcome::new(31, 50, "oracle/deterministic").blinded()),
        );
        outcomes
    }

    #[test]
    fn a_report_carries_cost_and_validity_together_for_every_contrast() {
        let design = design().with_contrast(Contrast::new("c/closure", "baseline", "no-closure"));
        let report = design.report(&scored_outcomes()).expect("reports");
        let contrast = &report.contrasts[0];
        assert_eq!(contrast.token_difference(), Some(-800));
        assert_eq!(
            contrast.variant.as_ref().and_then(|arm| arm.validity.as_ref()).map(|v| v.valid),
            Some(31)
        );
    }

    #[test]
    fn a_token_difference_between_two_estimators_is_not_reported_as_a_saving() {
        let design = design().with_contrast(Contrast::new("c/closure", "baseline", "no-closure"));
        let mut outcomes = scored_outcomes();
        outcomes.get_mut("no-closure").expect("present").cost =
            TokenEstimate::from_provider(400, "cl100k");
        let report = design.report(&outcomes).expect("reports");
        assert_eq!(report.contrasts[0].token_difference(), None);
    }

    #[test]
    fn a_report_states_whether_the_design_has_more_than_one_independent_parent() {
        let design = design().with_contrast(Contrast::new("c/closure", "baseline", "no-closure"));
        let report = design.report(&scored_outcomes()).expect("reports");
        assert!(!report.has_replication());
        assert_eq!(report.independent_clusters.len(), 1);
    }

    #[test]
    fn a_refusal_explains_what_would_have_to_change_to_get_an_answer() {
        let confounded = design()
            .with_arm(
                base_arm("two-off")
                    .set(ContextComponent::MandatoryClosure, "off")
                    .set(ContextComponent::RoleProjection, "on"),
            )
            .with_contrast(Contrast::new("c/two", "baseline", "two-off"));
        let mut outcomes = scored_outcomes();
        outcomes.insert(
            "two-off".to_string(),
            ArmOutcome::new("two-off", est(300)).scored(ValidityOutcome::new(20, 50, "oracle")),
        );
        let report = confounded.report(&outcomes).expect("reports");
        let explanation = report.explain_refusals();
        assert_eq!(explanation.len(), 1);
        assert!(explanation[0].contains("mandatory_closure"));
        assert!(explanation[0].contains("role_projection"));
        assert!(explanation[0].contains("one contrast per component"));
    }

    #[test]
    fn judging_is_deterministic_when_a_contrast_has_more_than_one_problem() {
        let broken = design().with_arm(
            AblationArm::new("messy", "cell/9")
                .set(ContextComponent::GraphCompilation, "off")
                .set(ContextComponent::MandatoryClosure, "off")
                .set(ContextComponent::ValueBasedExpansion, "greedy")
                .set(ContextComponent::DeltaContext, "off")
                .set(ContextComponent::RoleProjection, "off"),
        );
        let contrast = Contrast::new("c/messy", "baseline", "messy");
        let first = broken.judge(&contrast).expect("judges");
        for _ in 0..8 {
            assert_eq!(broken.judge(&contrast).expect("judges"), first);
        }
        assert!(matches!(
            first,
            ContrastVerdict::Refused {
                reason: ConfoundReason::ClusterMismatch { .. }
            }
        ));
    }

    #[test]
    fn an_ablation_report_survives_a_json_round_trip() {
        let design = design().with_contrast(Contrast::new("c/closure", "baseline", "no-closure"));
        let report = design.report(&scored_outcomes()).expect("reports");
        let text = serde_json::to_string(&report).expect("serialises");
        let back: AblationReport = serde_json::from_str(&text).expect("deserialises");
        assert_eq!(back, report);
    }
}
