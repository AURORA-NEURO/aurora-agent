//! Error-message quality as a checkable property.
//!
//! Every §11 module closes its failure section with the same clause — *emit an actionable
//! diagnostic* — and §11 never defines "actionable", so twenty-five modules assert a quality bar
//! that no test can fail. This module is that bar, written as rules over [`Diagnostic`] values.
//!
//! The three questions each rule serves:
//!
//! 1. **Does it name the rule?** A diagnostic whose `invariant` is "invalid input" has told the
//!    reader the class, not the rule. [`LintRule::UnnamedInvariant`].
//! 2. **Does it name the remedy?** [`LintRule::MissingRemedy`], and the two that stop a remedy
//!    from being decorative: [`LintRule::RemedyWithoutVerification`] and
//!    [`LintRule::UnclassifiedChange`].
//! 3. **Does it avoid claiming certainty it does not have?**
//!    [`LintRule::RemedyOverclaimsItsFinding`] and [`LintRule::CertaintyContradictsProse`].
//!
//! # Two severities, and why the split is not cosmetic
//!
//! [`LintSeverity::Error`] rules are decidable from structure: a field is empty, a confidence
//! ordering is violated, a required change is unclassified. A reviewer disagreeing with an error
//! is disagreeing about a fact.
//!
//! [`LintSeverity::Warning`] rules are heuristics over English prose, and this module says so
//! rather than dressing them up. [`LintRule::RemedyIsNotAnInstruction`] matches an opening
//! imperative verb against a fixed list; a perfectly good remedy phrased around an unusual verb
//! trips it. Promoting a heuristic to an error is how a lint teaches contributors to write for the
//! lint instead of for the reader.
//!
//! # This module grades its own crate
//!
//! [`crate::catalogue`](mod@crate::catalogue) is linted by [`lint_catalogue`] in this crate's own test suite, and the
//! results — including the warnings this crate does not fix — are recorded in the crate docs. A
//! lint whose author's own catalogue is exempt is a lint for other people.
//!
//! # Not implemented
//!
//! No spelling, grammar, tone or reading-level checks, and no natural-language model. Nothing here
//! parses English; it matches lowercase substrings and checks structural fields. That ceiling is
//! the reason for the two-severity split, not an oversight to be fixed later.

use crate::diagnostic::{Certainty, Diagnostic, ExplanationRequirement, Site};
use crate::taxonomy::ChangeRequired;
use bioprism_sdk::KNOWN_LOSS_KINDS;
use serde::{Deserialize, Serialize};

/// Phrases that name a class rather than a rule.
///
/// A diagnostic whose entire `invariant` is one of these has restated its own taxonomy entry.
const VAGUE_INVARIANTS: [&str; 8] = [
    "invalid input",
    "invalid",
    "error",
    "failed",
    "failure",
    "something went wrong",
    "unexpected",
    "internal error",
];

/// Hedges that contradict an `Observed` claim.
///
/// "may" is deliberately absent: it is the ordinary modal in this workspace's prose ("may
/// participate in a sufficiency claim") and including it would produce more false positives than
/// findings.
const HEDGES: [&str; 8] = [
    "probably",
    "might",
    "maybe",
    "perhaps",
    "possibly",
    "seems",
    "likely",
    "appears to",
];

/// Openers this lint recognises as instructions.
///
/// A fixed list, and therefore incomplete by construction. See the module docs on why
/// [`LintRule::RemedyIsNotAnInstruction`] is a warning.
const IMPERATIVE_OPENERS: [&str; 25] = [
    "add",
    "remove",
    "raise",
    "lower",
    "set",
    "declare",
    "drop",
    "widen",
    "narrow",
    "re-read",
    "reread",
    "re-run",
    "rerun",
    "regenerate",
    "replace",
    "rename",
    "pin",
    "obtain",
    "grant",
    "move",
    "split",
    "give",
    "keep",
    "run",
    "report",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LintSeverity {
    /// Decidable from structure. A diagnostic tripping one of these is defective.
    Error,
    /// A prose heuristic. A finding is a prompt to look, not a verdict.
    Warning,
}

impl LintSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            LintSeverity::Error => "error",
            LintSeverity::Warning => "warning",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LintRule {
    /// No remedy at all. The rule this crate exists for.
    MissingRemedy,
    /// A remedy that does not say how the developer would know it worked.
    RemedyWithoutVerification,
    /// A remedy that does not say which surface it touches.
    UnclassifiedChange,
    /// An `invariant` that names the class instead of the rule.
    UnnamedInvariant,
    /// An empty `observed`: the reader is told a rule was violated and not what was seen.
    UnnamedObservation,
    /// A remedy asserted more confidently than the finding it repairs.
    RemedyOverclaimsItsFinding,
    /// An unlocated site with no explanation of why nothing narrower was available.
    UnexplainedUnlocatedSite,
    /// A universal 23.32 explanation requirement left undischarged.
    UnmetExplanationRequirement,
    /// Prose that hedges while the diagnostic claims to have observed the fact.
    CertaintyContradictsProse,
    /// A semantic-loss kind outside `bioprism_sdk::KNOWN_LOSS_KINDS`.
    UnrecognisedLossKind,
    /// No blueprint module cited, so the invariant's source cannot be checked.
    UncitedInvariant,
    /// A remedy that does not open with a recognised imperative verb.
    RemedyIsNotAnInstruction,
}

impl LintRule {
    pub const ALL: [LintRule; 12] = [
        LintRule::MissingRemedy,
        LintRule::RemedyWithoutVerification,
        LintRule::UnclassifiedChange,
        LintRule::UnnamedInvariant,
        LintRule::UnnamedObservation,
        LintRule::RemedyOverclaimsItsFinding,
        LintRule::UnexplainedUnlocatedSite,
        LintRule::UnmetExplanationRequirement,
        LintRule::CertaintyContradictsProse,
        LintRule::UnrecognisedLossKind,
        LintRule::UncitedInvariant,
        LintRule::RemedyIsNotAnInstruction,
    ];

    pub fn severity(self) -> LintSeverity {
        match self {
            LintRule::MissingRemedy
            | LintRule::RemedyWithoutVerification
            | LintRule::UnclassifiedChange
            | LintRule::UnnamedInvariant
            | LintRule::UnnamedObservation
            | LintRule::RemedyOverclaimsItsFinding
            | LintRule::UnexplainedUnlocatedSite
            | LintRule::UnmetExplanationRequirement => LintSeverity::Error,
            LintRule::CertaintyContradictsProse
            | LintRule::UnrecognisedLossKind
            | LintRule::UncitedInvariant
            | LintRule::RemedyIsNotAnInstruction => LintSeverity::Warning,
        }
    }

    /// Whether the rule is decided by structure rather than by matching prose.
    pub fn is_structural(self) -> bool {
        self.severity() == LintSeverity::Error
    }

    pub fn as_str(self) -> &'static str {
        match self {
            LintRule::MissingRemedy => "missing_remedy",
            LintRule::RemedyWithoutVerification => "remedy_without_verification",
            LintRule::UnclassifiedChange => "unclassified_change",
            LintRule::UnnamedInvariant => "unnamed_invariant",
            LintRule::UnnamedObservation => "unnamed_observation",
            LintRule::RemedyOverclaimsItsFinding => "remedy_overclaims_its_finding",
            LintRule::UnexplainedUnlocatedSite => "unexplained_unlocated_site",
            LintRule::UnmetExplanationRequirement => "unmet_explanation_requirement",
            LintRule::CertaintyContradictsProse => "certainty_contradicts_prose",
            LintRule::UnrecognisedLossKind => "unrecognised_loss_kind",
            LintRule::UncitedInvariant => "uncited_invariant",
            LintRule::RemedyIsNotAnInstruction => "remedy_is_not_an_instruction",
        }
    }
}

/// One thing the lint found.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LintFinding {
    pub rule: LintRule,
    pub severity: LintSeverity,
    /// The diagnostic code, or the empty string if the diagnostic has none.
    pub code: String,
    pub detail: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LintReport {
    pub checked: usize,
    pub findings: Vec<LintFinding>,
}

impl LintReport {
    pub fn errors(&self) -> Vec<&LintFinding> {
        self.of_severity(LintSeverity::Error)
    }

    pub fn warnings(&self) -> Vec<&LintFinding> {
        self.of_severity(LintSeverity::Warning)
    }

    pub fn of_severity(&self, severity: LintSeverity) -> Vec<&LintFinding> {
        self.findings
            .iter()
            .filter(|f| f.severity == severity)
            .collect()
    }

    pub fn of_rule(&self, rule: LintRule) -> Vec<&LintFinding> {
        self.findings.iter().filter(|f| f.rule == rule).collect()
    }

    /// Clean means no errors. Warnings are prompts and do not gate.
    pub fn is_clean(&self) -> bool {
        self.errors().is_empty()
    }
}

fn contains_any(haystack: &str, needles: &[&str]) -> Option<String> {
    let lowered = haystack.to_lowercase();
    needles
        .iter()
        .find(|needle| lowered.contains(*needle))
        .map(|needle| (*needle).to_string())
}

fn opens_with_an_imperative(action: &str) -> bool {
    let lowered = action.trim().to_lowercase();
    IMPERATIVE_OPENERS
        .iter()
        .any(|verb| lowered == *verb || lowered.starts_with(&format!("{verb} ")))
}

/// Lint one diagnostic.
pub fn lint_diagnostic(diagnostic: &Diagnostic) -> Vec<LintFinding> {
    let code = diagnostic.code.as_str().to_string();
    let mut findings = Vec::new();
    let mut push = |rule: LintRule, detail: String| {
        findings.push(LintFinding {
            rule,
            severity: rule.severity(),
            code: code.clone(),
            detail,
        });
    };

    if diagnostic.remedies.is_empty() && diagnostic.class.remedy_is_mandatory() {
        push(
            LintRule::MissingRemedy,
            format!(
                "class {} requires a remedy and none is stated; the reader is told what broke and \
                 not what to do",
                diagnostic.class
            ),
        );
    }

    for (index, remedy) in diagnostic.remedies.iter().enumerate() {
        if remedy.verified_by.trim().is_empty() {
            push(
                LintRule::RemedyWithoutVerification,
                format!("remedy {index} states no way to tell whether it worked"),
            );
        }
        if remedy.change_required == ChangeRequired::Unknown {
            push(
                LintRule::UnclassifiedChange,
                format!("remedy {index} does not say which surface it touches"),
            );
        }
        if remedy
            .confidence
            .at_least_as_strong_as(diagnostic.certainty)
            && remedy.confidence != diagnostic.certainty
        {
            push(
                LintRule::RemedyOverclaimsItsFinding,
                format!(
                    "remedy {index} is asserted as {} while the finding it repairs is only {}",
                    remedy.confidence, diagnostic.certainty
                ),
            );
        }
        if !opens_with_an_imperative(&remedy.action) {
            push(
                LintRule::RemedyIsNotAnInstruction,
                format!(
                    "remedy {index} opens with {:?}, which is not in this lint's imperative list; \
                     the rule is a heuristic and this may be a false positive",
                    remedy.action.split_whitespace().next().unwrap_or("")
                ),
            );
        }
    }

    let invariant = diagnostic.invariant.trim().to_lowercase();
    if !invariant.is_empty() && VAGUE_INVARIANTS.contains(&invariant.as_str()) {
        push(
            LintRule::UnnamedInvariant,
            format!(
                "{:?} names the class, not the rule that was violated",
                diagnostic.invariant
            ),
        );
    }

    if diagnostic.observed.trim().is_empty() {
        push(
            LintRule::UnnamedObservation,
            "the diagnostic states a violated rule and not what was seen".to_string(),
        );
    }

    if let Site::Unlocated { because } = &diagnostic.site {
        if because.trim().len() < 10 {
            push(
                LintRule::UnexplainedUnlocatedSite,
                "an unlocated site must say why nothing narrower was available".to_string(),
            );
        }
    }

    for requirement in diagnostic.unmet_universal_requirements() {
        if requirement == ExplanationRequirement::PossibleSafeFixes {
            continue;
        }
        push(
            LintRule::UnmetExplanationRequirement,
            format!(
                "23.32 requires a diagnostic to explain {:?}; field `{}` is empty",
                requirement.phrase(),
                requirement.field()
            ),
        );
    }

    if diagnostic.certainty == Certainty::Observed {
        let prose = format!("{} {}", diagnostic.invariant, diagnostic.observed);
        if let Some(hedge) = contains_any(&prose, &HEDGES) {
            push(
                LintRule::CertaintyContradictsProse,
                format!("claims to have observed the fact while hedging with {hedge:?}"),
            );
        }
    }

    for kind in &diagnostic.semantic_loss {
        if !KNOWN_LOSS_KINDS.contains(&kind.as_str()) {
            push(
                LintRule::UnrecognisedLossKind,
                format!(
                    "{kind:?} is not in bioprism_sdk::KNOWN_LOSS_KINDS; that list is itself \
                     advisory, so this is a prompt to check for drift"
                ),
            );
        }
    }

    if diagnostic.blueprint_modules.is_empty() {
        push(
            LintRule::UncitedInvariant,
            "no blueprint module is cited, so the invariant's source cannot be checked".to_string(),
        );
    }

    findings.sort();
    findings
}

/// Lint a set of diagnostics.
pub fn lint(diagnostics: &[Diagnostic]) -> LintReport {
    let mut findings: Vec<LintFinding> = diagnostics.iter().flat_map(lint_diagnostic).collect();
    findings.sort();
    LintReport {
        checked: diagnostics.len(),
        findings,
    }
}

/// Lint this crate's own catalogue.
pub fn lint_catalogue() -> LintReport {
    lint(&crate::catalogue::catalogue())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::{DiagnosticCode, Remedy};
    use crate::taxonomy::DiagnosticClass;

    fn base() -> Diagnostic {
        Diagnostic::new(
            DiagnosticCode::parse("DEVX-0001").expect("code"),
            DiagnosticClass::ContractViolation,
            "the mandatory closure is a subset of the selection",
            "3 protected facts were dropped to fit a 12-fact budget",
            Site::Artifact {
                node_kind: "query".into(),
                id: "q-1".into(),
            },
        )
        .citing("43.13")
    }

    fn good_remedy() -> Remedy {
        Remedy::new(
            "raise budget_facts to at least 15",
            Site::Source {
                document: "query.json".into(),
                span: None,
            },
            "the compile emits no dropped_protected entry",
            ChangeRequired::Contract,
            Certainty::Observed,
        )
    }

    #[test]
    fn a_diagnostic_with_no_stated_remedy_fails_the_catalogue_lint() {
        let report = lint(&[base()]);
        assert!(!report.is_clean());
        assert_eq!(report.of_rule(LintRule::MissingRemedy).len(), 1);
    }

    #[test]
    fn a_remedy_that_cannot_be_verified_is_an_error() {
        let mut remedy = good_remedy();
        remedy.verified_by = "  ".into();
        let report = lint(&[base().with_remedy(remedy)]);
        assert_eq!(report.of_rule(LintRule::RemedyWithoutVerification).len(), 1);
    }

    #[test]
    fn a_remedy_on_an_unclassified_surface_is_an_error() {
        let mut remedy = good_remedy();
        remedy.change_required = ChangeRequired::Unknown;
        let report = lint(&[base().with_remedy(remedy)]);
        assert_eq!(report.of_rule(LintRule::UnclassifiedChange).len(), 1);
    }

    #[test]
    fn an_invariant_that_names_the_class_instead_of_the_rule_is_an_error() {
        let mut diagnostic = base().with_remedy(good_remedy());
        diagnostic.invariant = "invalid input".into();
        let report = lint(&[diagnostic]);
        assert_eq!(report.of_rule(LintRule::UnnamedInvariant).len(), 1);
    }

    #[test]
    fn an_empty_invariant_is_reported_as_an_unmet_2332_requirement_and_not_as_a_vague_one() {
        let mut diagnostic = base().with_remedy(good_remedy());
        diagnostic.invariant = "   ".into();
        let report = lint(&[diagnostic]);
        assert_eq!(
            report.of_rule(LintRule::UnmetExplanationRequirement).len(),
            1
        );
        assert!(report.of_rule(LintRule::UnnamedInvariant).is_empty());
    }

    #[test]
    fn a_missing_remedy_is_reported_once_and_not_also_as_an_unmet_requirement() {
        let report = lint(&[base()]);
        assert_eq!(report.of_rule(LintRule::MissingRemedy).len(), 1);
        assert!(report
            .of_rule(LintRule::UnmetExplanationRequirement)
            .is_empty());
    }

    #[test]
    fn a_remedy_asserted_more_confidently_than_its_finding_is_an_error() {
        let mut remedy = good_remedy();
        remedy.confidence = Certainty::Observed;
        let report = lint(&[base()
            .with_certainty(Certainty::Suspected)
            .with_remedy(remedy)]);
        assert_eq!(report.of_rule(LintRule::RemedyOverclaimsItsFinding).len(), 1);
    }

    #[test]
    fn a_remedy_weaker_than_its_finding_is_allowed() {
        let mut remedy = good_remedy();
        remedy.confidence = Certainty::Suspected;
        let report = lint(&[base().with_remedy(remedy)]);
        assert!(report.of_rule(LintRule::RemedyOverclaimsItsFinding).is_empty());
    }

    #[test]
    fn hedged_prose_under_an_observed_claim_is_a_warning_not_an_error() {
        let mut diagnostic = base().with_remedy(good_remedy());
        diagnostic.observed = "the budget is probably below the closure".into();
        let report = lint(&[diagnostic]);
        assert_eq!(report.of_rule(LintRule::CertaintyContradictsProse).len(), 1);
        assert!(report.is_clean());
    }

    #[test]
    fn the_same_hedge_under_a_suspected_claim_is_not_reported() {
        let mut diagnostic = base()
            .with_certainty(Certainty::Suspected)
            .with_remedy(good_remedy());
        diagnostic.observed = "the budget is probably below the closure".into();
        let report = lint(&[diagnostic]);
        assert!(report.of_rule(LintRule::CertaintyContradictsProse).is_empty());
    }

    #[test]
    fn an_unlocated_site_without_an_explanation_is_an_error() {
        let mut diagnostic = base().with_remedy(good_remedy());
        diagnostic.site = Site::Unlocated {
            because: "dunno".into(),
        };
        let report = lint(&[diagnostic]);
        assert_eq!(report.of_rule(LintRule::UnexplainedUnlocatedSite).len(), 1);
    }

    #[test]
    fn a_loss_kind_outside_the_sdk_list_is_a_warning() {
        let report = lint(&[base()
            .with_remedy(good_remedy())
            .with_semantic_loss("invented_kind")]);
        assert_eq!(report.of_rule(LintRule::UnrecognisedLossKind).len(), 1);
        assert!(report.is_clean());
    }

    #[test]
    fn a_known_loss_kind_is_not_reported() {
        let report = lint(&[base()
            .with_remedy(good_remedy())
            .with_semantic_loss(KNOWN_LOSS_KINDS[0])]);
        assert!(report.of_rule(LintRule::UnrecognisedLossKind).is_empty());
    }

    #[test]
    fn every_error_rule_is_structural_and_every_warning_rule_is_a_prose_heuristic() {
        for rule in LintRule::ALL {
            assert_eq!(rule.is_structural(), rule.severity() == LintSeverity::Error);
        }
        assert_eq!(
            LintRule::ALL
                .iter()
                .filter(|r| r.severity() == LintSeverity::Warning)
                .count(),
            4
        );
    }

    #[test]
    fn a_well_formed_diagnostic_produces_no_findings_at_all() {
        let report = lint(&[base().with_remedy(good_remedy())]);
        assert!(report.findings.is_empty(), "{:?}", report.findings);
        assert_eq!(report.checked, 1);
    }
}
