//! A diagnostic a machine can act on.
//!
//! Blueprint 23.32's diagnostics clause is the specification this module implements literally. It
//! says an error should explain: what contract failed; source location; relevant role and state;
//! current and required types/effects; why an adapter is lossy; possible safe fixes; whether a
//! human decision is required. Seven requirements, each mapped to a field by
//! [`ExplanationRequirement::field`] and checked by a test rather than by a reviewer's memory.
//!
//! # The load-bearing field is [`Diagnostic::remedies`]
//!
//! Every §11 module ends its failure clause with the same sentence — *emit an actionable
//! diagnostic rather than silently repairing or discarding state* — and repeats it twenty-five
//! times without once saying what makes a diagnostic actionable. This crate answers: a diagnostic
//! is actionable when it states **what would have to change for the check to pass**, at a named
//! site, with a stated way to tell whether the change worked.
//!
//! `bioprism-examples` already enforces the same rule one layer up, on blocked property claims: a
//! blocked claim records the concrete obstacle ("`WorldSpec::LeakageMechanism` has four members;
//! 38.01 names six") rather than "unsupported". [`crate::lint`](mod@crate::lint) applies it to
//! diagnostics, and a diagnostic with no remedy fails that lint.
//!
//! # Certainty is a field because a confident wrong remedy is worse than none
//!
//! A tool that says "add the `--budget` flag" when the real cause is elsewhere costs a developer
//! more than a tool that says "the budget is the most likely cause; I did not verify it".
//! [`Certainty`] is carried on the diagnostic *and* separately on each remedy, and the lint
//! rejects a remedy asserted more confidently than the finding it repairs.
//!
//! # Not implemented
//!
//! No rendering. There is no terminal here, no colour, no width-aware wrapping and no `Display`
//! that pretends to be a compiler's error output. A diagnostic is a serialisable record; how it is
//! drawn belongs to whatever is drawing. No filesystem either: a [`Site`] carries a document name
//! and a line span as *data*, and nothing in this crate opens it.

use crate::error::CodeError;
use crate::taxonomy::{ChangeRequired, DiagnosticClass, Retryability};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// A stable diagnostic identifier of the form `DEVX-0001`.
///
/// Checked at construction because the code is the join key between a catalogue entry, a lint
/// finding and an exit-code audit row. A code whose shape varies cannot be matched by a consumer
/// that is not this crate.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DiagnosticCode(String);

impl DiagnosticCode {
    pub fn parse(value: impl Into<String>) -> Result<Self, CodeError> {
        let text: String = value.into();
        if text.is_empty() {
            return Err(CodeError::Empty);
        }
        let Some(digits) = text.strip_prefix("DEVX-") else {
            return Err(CodeError::Malformed { code: text });
        };
        if digits.len() != 4 || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return Err(CodeError::Malformed { code: text });
        }
        if digits == "0000" {
            return Err(CodeError::OutOfRange { code: text });
        }
        Ok(DiagnosticCode(text))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A 1-based, inclusive line span in a named document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LineSpan {
    pub start: u32,
    pub end: u32,
}

impl LineSpan {
    /// A single line.
    pub fn at(line: u32) -> Self {
        LineSpan {
            start: line,
            end: line,
        }
    }

    pub fn new(start: u32, end: u32) -> Self {
        LineSpan {
            start: start.min(end),
            end: start.max(end),
        }
    }

    pub fn lines(&self) -> u32 {
        self.end - self.start + 1
    }
}

/// Where the violated invariant lives.
///
/// 23.32 asks for "source location". In an agent-facing platform that is not always a file and a
/// line: half of what fails here fails inside a compiled artefact that no editor can open, and a
/// diagnostic that invents a file path for it is lying about where to look. The variants are the
/// four kinds of address this workspace actually has.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Site {
    /// A named document and optionally a line span. The name is data; nothing here opens it.
    Source {
        document: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        span: Option<LineSpan>,
    },
    /// A node inside a compiled artefact: a fact id, a factor id, a pass name.
    Artifact { node_kind: String, id: String },
    /// A content-addressed blob. The only site that is stable across machines and checkouts.
    Digest { digest: String },
    /// The invocation itself: a flag, a positional argument, a subcommand.
    Invocation { argument: String },
    /// The site is genuinely not known.
    ///
    /// Representable on purpose. A diagnostic that guesses a location so the field can be filled
    /// sends a developer to the wrong place, which is strictly worse than admitting the gap; the
    /// `because` string has to say why nothing narrower was available.
    Unlocated { because: String },
}

impl Site {
    /// A short address for logs and reports.
    pub fn describe(&self) -> String {
        match self {
            Site::Source {
                document,
                span: Some(span),
            } if span.start == span.end => format!("{document}:{}", span.start),
            Site::Source {
                document,
                span: Some(span),
            } => format!("{document}:{}-{}", span.start, span.end),
            Site::Source {
                document,
                span: None,
            } => document.clone(),
            Site::Artifact { node_kind, id } => format!("{node_kind}/{id}"),
            Site::Digest { digest } => format!("sha256:{digest}"),
            Site::Invocation { argument } => format!("argv:{argument}"),
            Site::Unlocated { .. } => "<unlocated>".to_string(),
        }
    }

    /// Whether the site points at something a reader can go and look at.
    pub fn is_addressable(&self) -> bool {
        !matches!(self, Site::Unlocated { .. })
    }
}

/// How strongly a statement is asserted.
///
/// The three levels are distinguished by *what was checked*, not by how the sentence reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Certainty {
    /// The tool checked this directly and holds the evidence.
    Observed,
    /// Derived from something checked, under an assumption the diagnostic states.
    Inferred,
    /// A heuristic. Offered because it is usually right, not because it was verified.
    Suspected,
}

impl Certainty {
    pub const ALL: [Certainty; 3] = [Certainty::Observed, Certainty::Inferred, Certainty::Suspected];

    pub fn as_str(self) -> &'static str {
        match self {
            Certainty::Observed => "observed",
            Certainty::Inferred => "inferred",
            Certainty::Suspected => "suspected",
        }
    }

    /// Rank, where a larger number is a weaker claim.
    ///
    /// Used only to compare a remedy against the finding it repairs; it is not a severity.
    pub fn weakness(self) -> u8 {
        match self {
            Certainty::Observed => 0,
            Certainty::Inferred => 1,
            Certainty::Suspected => 2,
        }
    }

    /// Whether `self` is asserted at least as strongly as `other`.
    pub fn at_least_as_strong_as(self, other: Certainty) -> bool {
        self.weakness() <= other.weakness()
    }
}

impl fmt::Display for Certainty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What would have to change for the violated invariant to hold.
///
/// The three required parts are the action, the site it applies to, and how the developer would
/// know it worked. The third is the one usually missing: a remedy with no observable consequence
/// cannot be distinguished from a remedy that did nothing, and an agent applying it has no
/// stopping condition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Remedy {
    /// The change, as an instruction. Imperative, concrete, and about the site named below.
    pub action: String,
    /// Where the change goes. Frequently *not* the site of the violation: a budget below the
    /// mandatory closure is observed in the compiler and fixed in the query.
    pub site: Site,
    /// What the developer should observe once the change is made.
    pub verified_by: String,
    /// Which surface this change touches, so a client can decide whether it may apply it.
    pub change_required: ChangeRequired,
    /// How strongly this remedy is asserted. Never stronger than the diagnostic's own certainty.
    pub confidence: Certainty,
}

impl Remedy {
    pub fn new(
        action: impl Into<String>,
        site: Site,
        verified_by: impl Into<String>,
        change_required: ChangeRequired,
        confidence: Certainty,
    ) -> Self {
        Remedy {
            action: action.into(),
            site,
            verified_by: verified_by.into(),
            change_required,
            confidence,
        }
    }
}

/// One thing 23.32 requires a diagnostic to explain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExplanationRequirement {
    WhatContractFailed,
    SourceLocation,
    RelevantRoleAndState,
    CurrentAndRequiredTypes,
    WhyAnAdapterIsLossy,
    PossibleSafeFixes,
    WhetherAHumanDecisionIsRequired,
}

impl ExplanationRequirement {
    pub const ALL: [ExplanationRequirement; 7] = [
        ExplanationRequirement::WhatContractFailed,
        ExplanationRequirement::SourceLocation,
        ExplanationRequirement::RelevantRoleAndState,
        ExplanationRequirement::CurrentAndRequiredTypes,
        ExplanationRequirement::WhyAnAdapterIsLossy,
        ExplanationRequirement::PossibleSafeFixes,
        ExplanationRequirement::WhetherAHumanDecisionIsRequired,
    ];

    /// The 23.32 phrase, so the mapping can be checked against the source text.
    pub fn phrase(self) -> &'static str {
        match self {
            ExplanationRequirement::WhatContractFailed => "what contract failed",
            ExplanationRequirement::SourceLocation => "source location",
            ExplanationRequirement::RelevantRoleAndState => "relevant role and state",
            ExplanationRequirement::CurrentAndRequiredTypes => "current and required types/effects",
            ExplanationRequirement::WhyAnAdapterIsLossy => "why an adapter is lossy",
            ExplanationRequirement::PossibleSafeFixes => "possible safe fixes",
            ExplanationRequirement::WhetherAHumanDecisionIsRequired => {
                "whether a human decision is required"
            }
        }
    }

    /// The [`Diagnostic`] field that discharges it.
    pub fn field(self) -> &'static str {
        match self {
            ExplanationRequirement::WhatContractFailed => "invariant",
            ExplanationRequirement::SourceLocation => "site",
            ExplanationRequirement::RelevantRoleAndState => "context",
            ExplanationRequirement::CurrentAndRequiredTypes => "discrepancy",
            ExplanationRequirement::WhyAnAdapterIsLossy => "semantic_loss",
            ExplanationRequirement::PossibleSafeFixes => "remedies",
            ExplanationRequirement::WhetherAHumanDecisionIsRequired => "human_decision_required",
        }
    }

    /// Whether a given diagnostic actually populated the field, as opposed to the type merely
    /// having one.
    ///
    /// The optional requirements are genuinely optional per diagnostic: not every failure involves
    /// an adapter or a type mismatch. What is *not* optional is
    /// [`WhatContractFailed`](ExplanationRequirement::WhatContractFailed),
    /// [`PossibleSafeFixes`](ExplanationRequirement::PossibleSafeFixes) and
    /// [`WhetherAHumanDecisionIsRequired`](ExplanationRequirement::WhetherAHumanDecisionIsRequired),
    /// which [`crate::lint`](mod@crate::lint) enforces.
    pub fn satisfied_by(self, diagnostic: &Diagnostic) -> bool {
        match self {
            ExplanationRequirement::WhatContractFailed => !diagnostic.invariant.trim().is_empty(),
            ExplanationRequirement::SourceLocation => diagnostic.site.is_addressable(),
            ExplanationRequirement::RelevantRoleAndState => !diagnostic.context.is_empty(),
            ExplanationRequirement::CurrentAndRequiredTypes => diagnostic.discrepancy.is_some(),
            ExplanationRequirement::WhyAnAdapterIsLossy => !diagnostic.semantic_loss.is_empty(),
            ExplanationRequirement::PossibleSafeFixes => !diagnostic.remedies.is_empty(),
            ExplanationRequirement::WhetherAHumanDecisionIsRequired => true,
        }
    }

    /// Requirements every diagnostic must discharge, whatever it is about.
    pub fn is_universal(self) -> bool {
        matches!(
            self,
            ExplanationRequirement::WhatContractFailed
                | ExplanationRequirement::PossibleSafeFixes
                | ExplanationRequirement::WhetherAHumanDecisionIsRequired
        )
    }
}

/// What the tool found versus what the contract demanded.
///
/// 23.32's "current and required types/effects", generalised: the same shape carries a version
/// range, an effect set, a budget or an arity, and keeping it as two strings avoids inventing a
/// type lattice this crate has no use for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Discrepancy {
    /// What the contract requires.
    pub required: String,
    /// What was actually present.
    pub current: String,
}

impl Discrepancy {
    pub fn new(required: impl Into<String>, current: impl Into<String>) -> Self {
        Discrepancy {
            required: required.into(),
            current: current.into(),
        }
    }
}

/// A machine-actionable developer diagnostic.
///
/// Constructed through [`Diagnostic::new`] and the builder methods, which fill nothing in by
/// default. Nothing here validates: an incomplete diagnostic must be *representable*, or
/// [`crate::lint`](mod@crate::lint) would have nothing to catch and the quality rule would degrade into a type
/// error that a contributor works around.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub class: DiagnosticClass,
    /// The rule that was violated, named as a rule. Not "invalid input" — the invariant.
    pub invariant: String,
    /// What was actually observed, in terms a reader can check against the artefact.
    pub observed: String,
    pub site: Site,
    /// What would have to change. Empty is representable and fails the lint.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remedies: Vec<Remedy>,
    pub certainty: Certainty,
    /// Whether a person, not an agent, has to decide. 23.32's seventh requirement, and the one an
    /// autonomous client most needs: it is the difference between retrying and escalating.
    pub human_decision_required: bool,
    /// Role, state and any other named context. 23.32's "relevant role and state".
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub context: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discrepancy: Option<Discrepancy>,
    /// Semantic-loss kinds implicated, spelled as `bioprism_sdk::KNOWN_LOSS_KINDS` spells them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub semantic_loss: Vec<String>,
    /// Blueprint modules that assert the violated invariant.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blueprint_modules: Vec<String>,
}

impl Diagnostic {
    pub fn new(
        code: DiagnosticCode,
        class: DiagnosticClass,
        invariant: impl Into<String>,
        observed: impl Into<String>,
        site: Site,
    ) -> Self {
        Diagnostic {
            code,
            class,
            invariant: invariant.into(),
            observed: observed.into(),
            site,
            remedies: Vec::new(),
            certainty: Certainty::Observed,
            human_decision_required: false,
            context: BTreeMap::new(),
            discrepancy: None,
            semantic_loss: Vec::new(),
            blueprint_modules: Vec::new(),
        }
    }

    pub fn with_remedy(mut self, remedy: Remedy) -> Self {
        self.remedies.push(remedy);
        self
    }

    pub fn with_certainty(mut self, certainty: Certainty) -> Self {
        self.certainty = certainty;
        self
    }

    pub fn needing_human_decision(mut self) -> Self {
        self.human_decision_required = true;
        self
    }

    pub fn with_context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.insert(key.into(), value.into());
        self
    }

    pub fn with_discrepancy(mut self, discrepancy: Discrepancy) -> Self {
        self.discrepancy = Some(discrepancy);
        self
    }

    pub fn with_semantic_loss(mut self, kind: impl Into<String>) -> Self {
        self.semantic_loss.push(kind.into());
        self
    }

    pub fn citing(mut self, module: impl Into<String>) -> Self {
        self.blueprint_modules.push(module.into());
        self
    }

    /// The retry decision, taken from the class so that it cannot drift per diagnostic.
    pub fn retryability(&self) -> Retryability {
        self.class.retryability()
    }

    /// The surface a caller must change.
    ///
    /// Taken from the remedies when they agree, and from the class otherwise. Remedies that
    /// disagree mean the diagnostic offers alternatives on different surfaces, which is legitimate
    /// — "raise the budget or drop the protected tag" — and in that case the class's answer is the
    /// conservative one.
    pub fn change_required(&self) -> ChangeRequired {
        let mut kinds = self.remedies.iter().map(|r| r.change_required);
        match kinds.next() {
            Some(first) if kinds.all(|k| k == first) => first,
            Some(_) => self.class.change_required(),
            None => ChangeRequired::Unknown,
        }
    }

    /// The 23.32 requirements this particular diagnostic discharges.
    pub fn explanation_coverage(&self) -> Vec<ExplanationRequirement> {
        ExplanationRequirement::ALL
            .into_iter()
            .filter(|req| req.satisfied_by(self))
            .collect()
    }

    /// Universal requirements this diagnostic left undischarged.
    pub fn unmet_universal_requirements(&self) -> Vec<ExplanationRequirement> {
        ExplanationRequirement::ALL
            .into_iter()
            .filter(|req| req.is_universal() && !req.satisfied_by(self))
            .collect()
    }

    /// The weakest certainty asserted anywhere in the diagnostic.
    pub fn weakest_claim(&self) -> Certainty {
        self.remedies
            .iter()
            .map(|r| r.confidence)
            .fold(self.certainty, |acc, next| {
                if next.weakness() > acc.weakness() {
                    next
                } else {
                    acc
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Diagnostic {
        Diagnostic::new(
            DiagnosticCode::parse("DEVX-0001").expect("well-formed code"),
            DiagnosticClass::ContractViolation,
            "the mandatory closure is a subset of the selection",
            "3 protected facts were dropped to fit a 12-fact budget",
            Site::Artifact {
                node_kind: "query".into(),
                id: "q-1".into(),
            },
        )
    }

    #[test]
    fn a_code_outside_the_grammar_is_rejected_at_construction() {
        assert!(DiagnosticCode::parse("").is_err());
        assert!(DiagnosticCode::parse("E0001").is_err());
        assert!(DiagnosticCode::parse("DEVX-1").is_err());
        assert!(DiagnosticCode::parse("DEVX-000a").is_err());
        assert!(DiagnosticCode::parse("DEVX-0000").is_err());
        assert!(DiagnosticCode::parse("DEVX-0042").is_ok());
    }

    #[test]
    fn a_diagnostic_with_no_remedy_reports_its_required_change_as_unknown() {
        assert_eq!(sample().change_required(), ChangeRequired::Unknown);
    }

    #[test]
    fn remedies_agreeing_on_a_surface_determine_the_required_change() {
        let diagnostic = sample().with_remedy(Remedy::new(
            "raise budget_facts to at least 15",
            Site::Source {
                document: "query.json".into(),
                span: Some(LineSpan::at(4)),
            },
            "the compile emits no dropped_protected entry",
            ChangeRequired::Contract,
            Certainty::Observed,
        ));
        assert_eq!(diagnostic.change_required(), ChangeRequired::Contract);
    }

    #[test]
    fn disagreeing_remedies_fall_back_to_the_class_answer() {
        let diagnostic = sample()
            .with_remedy(Remedy::new(
                "raise budget_facts to at least 15",
                Site::Source {
                    document: "query.json".into(),
                    span: None,
                },
                "no dropped_protected entry",
                ChangeRequired::Contract,
                Certainty::Observed,
            ))
            .with_remedy(Remedy::new(
                "drop the protected tag `split_assignment`",
                Site::Source {
                    document: "query.json".into(),
                    span: None,
                },
                "the protected closure shrinks below the budget",
                ChangeRequired::Payload,
                Certainty::Inferred,
            ));
        assert_eq!(
            diagnostic.change_required(),
            DiagnosticClass::ContractViolation.change_required()
        );
    }

    #[test]
    fn an_unlocated_site_does_not_satisfy_the_source_location_requirement() {
        let diagnostic = Diagnostic::new(
            DiagnosticCode::parse("DEVX-0002").expect("code"),
            DiagnosticClass::Internal,
            "every pass records an outcome",
            "a pass produced no record",
            Site::Unlocated {
                because: "the trace was truncated before the pass name was written".into(),
            },
        );
        assert!(!ExplanationRequirement::SourceLocation.satisfied_by(&diagnostic));
        assert_eq!(diagnostic.site.describe(), "<unlocated>");
    }

    #[test]
    fn the_weakest_claim_is_taken_over_the_diagnostic_and_all_its_remedies() {
        let diagnostic = sample().with_remedy(Remedy::new(
            "raise the budget",
            Site::Invocation {
                argument: "--budget".into(),
            },
            "the compile succeeds",
            ChangeRequired::Contract,
            Certainty::Suspected,
        ));
        assert_eq!(diagnostic.certainty, Certainty::Observed);
        assert_eq!(diagnostic.weakest_claim(), Certainty::Suspected);
    }

    #[test]
    fn a_diagnostic_round_trips_through_json_with_its_site_tagged() {
        let diagnostic = sample().with_context("role", "compiler");
        let encoded = serde_json::to_value(&diagnostic).expect("serialises");
        assert_eq!(encoded["site"]["kind"], "artifact");
        let decoded: Diagnostic = serde_json::from_value(encoded).expect("parses back");
        assert_eq!(diagnostic, decoded);
    }

    #[test]
    fn line_spans_normalise_a_reversed_range() {
        assert_eq!(LineSpan::new(9, 4), LineSpan::new(4, 9));
        assert_eq!(LineSpan::new(4, 9).lines(), 6);
        assert_eq!(LineSpan::at(7).lines(), 1);
    }
}
