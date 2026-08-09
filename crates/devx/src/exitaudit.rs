//! The exit-code audit: what `bioprism-cli` ships versus what the taxonomy needs.
//!
//! Blueprint 11.03 (CLI specification) and 40.13 both require errors to map to documented exit
//! codes, and 40.36 requires the retry classification to survive that mapping. `bioprism-cli`
//! ships six codes. [`crate::taxonomy`] has nine classes. Six into nine does not go, and this
//! module says exactly where it fails and what it costs.
//!
//! # Why the registry is transcribed rather than imported
//!
//! `bioprism-cli` is a binary crate and is not in this crate's dependency set, so
//! [`shipped_exit_codes`] is a *copy* of `crates/cli/src/exit.rs` as of the revision this module
//! was written against, and a copy can drift. That is stated here rather than hidden behind an
//! import that does not exist: [`SHIPPED_REGISTRY_SOURCE`] names the file a reviewer must open to
//! check the transcription, and [`SHIPPED_REGISTRY_TRANSCRIBED_FIELDS`] names what was copied. The
//! alternative — auditing nothing because the dependency edge is unavailable — leaves a known
//! defect unreported, which is worse.
//!
//! # The findings, in short
//!
//! Two defects and five imprecisions. The defects:
//!
//! - **Exit 4 carries five classes.** `Stale`, `Conflict`, `PolicyDenied`, `ContractViolation` and
//!   `Indeterminate` all exit 4. A CI script branching on the exit code cannot tell a policy
//!   refusal from an oracle abstention from a snapshot that moved under it, and those three want
//!   three different next actions.
//! - **`Stale` is advertised as terminal and is not.** Exit 4's `is_retryable()` is `false`, but a
//!   stale precondition is [`Retryability::RetryableAsIs`] after a re-read. A client that trusts
//!   the code stops when it should have re-read and re-sent; this is the only place in the
//!   registry where the advertised retry decision is the *opposite* of the true one, rather than
//!   merely coarser.
//!
//! Both were first named by `bioprism-services`. This module reproduces them from an independent
//! table, adds the four narrowing findings and one note, and states each as a [`Diagnostic`] with
//! a remedy, so that the audit is actionable rather than a complaint.
//!
//! # What is deliberately not claimed
//!
//! Exit code 1, `AssertionFailed`, has no class in the taxonomy, and this module reports that as a
//! **note, not a defect**. 40.36 classifies failures; exit 1 is a completed run whose checked
//! property did not hold, which is a verdict. Filing it as a taxonomy gap would be a fabricated
//! finding, and an audit that inflates its own count is one nobody reads twice.
//!
//! Nothing here proposes new numeric codes. Choosing them is `bioprism-cli`'s call and this crate
//! cannot edit that file; what it can do is state the distinctions any replacement must preserve.

use crate::diagnostic::{Certainty, Diagnostic, DiagnosticCode, Remedy, Site};
use crate::taxonomy::{ChangeRequired, DiagnosticClass, Retryability};
use serde::{Deserialize, Serialize};

/// The file a reviewer opens to check the transcription below.
pub const SHIPPED_REGISTRY_SOURCE: &str = "crates/cli/src/exit.rs";

/// The fields copied out of that file. Anything else about the CLI is not modelled here.
pub const SHIPPED_REGISTRY_TRANSCRIBED_FIELDS: [&str; 3] =
    ["ExitCode discriminant", "ExitCode::slug", "ExitCode::is_retryable"];

/// One row of `bioprism-cli`'s shipped registry, transcribed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShippedExitCode {
    pub code: u8,
    /// `ExitCode::slug`.
    pub slug: String,
    /// The doc comment's meaning, condensed. What a reader of the CLI would believe the code says.
    pub meaning: String,
    /// `ExitCode::is_retryable`, which is `true` for code 5 alone.
    pub advertised_retryable: bool,
}

/// The six codes `bioprism-cli` ships.
pub fn shipped_exit_codes() -> Vec<ShippedExitCode> {
    let rows: [(u8, &str, &str, bool); 6] = [
        (0, "ok", "the command completed and its assertion held", false),
        (
            1,
            "assertion_failed",
            "the command completed but the thing it checked did not hold",
            false,
        ),
        (2, "usage", "bad invocation", false),
        (
            3,
            "invalid_input",
            "input did not satisfy its schema or could not be parsed",
            false,
        ),
        (
            4,
            "compile_failed",
            "compilation could not produce a sound result within the declared contract",
            false,
        ),
        (5, "io", "a file could not be read or written", true),
    ];
    rows.into_iter()
        .map(|(code, slug, meaning, retryable)| ShippedExitCode {
            code,
            slug: slug.to_string(),
            meaning: meaning.to_string(),
            advertised_retryable: retryable,
        })
        .collect()
}

/// The code `bioprism-cli` would return for a diagnostic class, given only the codes it has.
///
/// This is a description of a forced choice, not a proposal. Five classes land on 4 because 4 is
/// the only code left that means "the run did not produce a result".
pub fn shipped_code_for(class: DiagnosticClass) -> u8 {
    match class {
        DiagnosticClass::Usage => 2,
        DiagnosticClass::InvalidInput => 3,
        DiagnosticClass::Stale
        | DiagnosticClass::Conflict
        | DiagnosticClass::PolicyDenied
        | DiagnosticClass::ContractViolation
        | DiagnosticClass::Indeterminate => 4,
        DiagnosticClass::Unavailable | DiagnosticClass::Internal => 5,
    }
}

/// Whether the code's stated meaning actually covers the class, or merely absorbs it.
///
/// This is the judgement in the module, and it is written where a reader can disagree with it. The
/// question asked of each pair is narrow: *would a developer reading only the code's doc comment
/// correctly describe what happened?* For `ContractViolation` at code 4 the answer is yes — the
/// doc comment's own example is a budget smaller than the protected closure. For `Indeterminate`
/// it is no: an oracle that ran correctly and returned "the evidence does not decide this" has not
/// suffered a compile failure, and 40.36 invariant 4 exists precisely to stop that conflation.
pub fn code_meaning_covers(class: DiagnosticClass) -> bool {
    match class {
        DiagnosticClass::Usage => true,
        DiagnosticClass::InvalidInput => true,
        DiagnosticClass::Stale => false,
        DiagnosticClass::Conflict => false,
        DiagnosticClass::PolicyDenied => false,
        DiagnosticClass::ContractViolation => true,
        DiagnosticClass::Indeterminate => false,
        DiagnosticClass::Unavailable => true,
        DiagnosticClass::Internal => false,
    }
}

/// How badly a divergence hurts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditSeverity {
    /// A consumer acting on the exit code will do the wrong thing.
    Defect,
    /// A consumer acting on the exit code will do a defensible thing for the wrong reason, or will
    /// be unable to act as precisely as the taxonomy would allow.
    Imprecision,
    /// An observation about the boundary between the two models. Not a fault in either.
    Note,
}

impl AuditSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            AuditSeverity::Defect => "defect",
            AuditSeverity::Imprecision => "imprecision",
            AuditSeverity::Note => "note",
        }
    }
}

/// What kind of divergence this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DivergenceKind {
    /// The code advertises a retry decision that contradicts the class's.
    RetryabilityInverted,
    /// One code carries several classes, so the classes cannot be distinguished downstream.
    ClassCollision,
    /// The code's stated meaning describes something narrower or simply other than the class.
    MeaningNarrowerThanClass,
    /// A shipped code that the taxonomy does not classify, because it is not a failure.
    CodeOutsideTheTaxonomy,
}

impl DivergenceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            DivergenceKind::RetryabilityInverted => "retryability_inverted",
            DivergenceKind::ClassCollision => "class_collision",
            DivergenceKind::MeaningNarrowerThanClass => "meaning_narrower_than_class",
            DivergenceKind::CodeOutsideTheTaxonomy => "code_outside_the_taxonomy",
        }
    }
}

/// One audit row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Divergence {
    pub kind: DivergenceKind,
    pub severity: AuditSeverity,
    pub code: u8,
    /// The classes involved, in taxonomy order.
    pub classes: Vec<DiagnosticClass>,
    /// What is wrong, stated so it can be checked against the two tables.
    pub finding: String,
    /// What a consumer loses. The reason this row is in the report rather than in a comment.
    pub consequence: String,
    /// The distinction a replacement registry must preserve.
    pub required_distinction: String,
}

/// The audit result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExitCodeAudit {
    pub registry_source: String,
    pub shipped_code_count: usize,
    pub class_count: usize,
    pub divergences: Vec<Divergence>,
}

impl ExitCodeAudit {
    pub fn defects(&self) -> Vec<&Divergence> {
        self.by_severity(AuditSeverity::Defect)
    }

    pub fn by_severity(&self, severity: AuditSeverity) -> Vec<&Divergence> {
        self.divergences
            .iter()
            .filter(|d| d.severity == severity)
            .collect()
    }

    pub fn is_clean(&self) -> bool {
        self.defects().is_empty()
    }

    /// The audit as diagnostics, so the audit itself is graded by
    /// [`crate::lint`](mod@crate::lint).
    ///
    /// An audit that reports a problem without stating a fix is exactly the failure this crate
    /// exists to lint against, so the audit does not get an exemption from its own rule.
    pub fn as_diagnostics(&self) -> Vec<Diagnostic> {
        self.divergences
            .iter()
            .enumerate()
            .map(|(index, divergence)| divergence_to_diagnostic(index, divergence))
            .collect()
    }
}

fn divergence_to_diagnostic(index: usize, divergence: &Divergence) -> Diagnostic {
    let code = DiagnosticCode::parse(format!("DEVX-9{:03}", index + 1))
        .expect("audit codes are generated in range");
    let class = match divergence.severity {
        AuditSeverity::Defect => DiagnosticClass::ContractViolation,
        AuditSeverity::Imprecision | AuditSeverity::Note => DiagnosticClass::Indeterminate,
    };
    let certainty = match divergence.kind {
        DivergenceKind::MeaningNarrowerThanClass => Certainty::Inferred,
        _ => Certainty::Observed,
    };
    let mut diagnostic = Diagnostic::new(
        code,
        class,
        format!(
            "an exit code preserves the retry decision and the class distinction of the failure it reports ({})",
            divergence.kind.as_str()
        ),
        divergence.finding.clone(),
        Site::Source {
            document: SHIPPED_REGISTRY_SOURCE.to_string(),
            span: None,
        },
    )
    .with_certainty(certainty)
    .with_context("exit_code", divergence.code.to_string())
    .with_context(
        "classes",
        divergence
            .classes
            .iter()
            .map(|c| c.as_str())
            .collect::<Vec<_>>()
            .join(","),
    )
    .with_context("consequence", divergence.consequence.clone())
    .citing("40.36")
    .citing("11.03")
    .with_remedy(Remedy::new(
        divergence.required_distinction.clone(),
        Site::Source {
            document: SHIPPED_REGISTRY_SOURCE.to_string(),
            span: None,
        },
        format!(
            "bioprism_devx::exitaudit::audit() no longer reports a {} row for exit {}",
            divergence.kind.as_str(),
            divergence.code
        ),
        ChangeRequired::Contract,
        certainty,
    ));
    if divergence.severity == AuditSeverity::Defect {
        diagnostic = diagnostic.needing_human_decision();
    }
    diagnostic
}

/// Run the audit.
///
/// Derived from [`shipped_code_for`] and [`code_meaning_covers`] rather than written out, so a
/// change to either table changes the report. Each class contributes at most one row, taking the
/// worst finding, because counting `Stale` twice would inflate the report without adding a fact.
pub fn audit() -> ExitCodeAudit {
    let shipped = shipped_exit_codes();
    let mut divergences = Vec::new();

    for class in DiagnosticClass::ALL {
        let code = shipped_code_for(class);
        let row = shipped
            .iter()
            .find(|row| row.code == code)
            .expect("every mapped code is in the shipped registry");
        let advertised_retryable = row.advertised_retryable;
        let truly_retryable = class.retryability() == Retryability::RetryableAsIs;

        if advertised_retryable != truly_retryable {
            divergences.push(Divergence {
                kind: DivergenceKind::RetryabilityInverted,
                severity: AuditSeverity::Defect,
                code,
                classes: vec![class],
                finding: format!(
                    "{class} maps to exit {code} ({}), whose is_retryable() is {advertised_retryable}, \
                     but {class} is {}",
                    row.slug,
                    class.retryability()
                ),
                consequence: if truly_retryable {
                    format!(
                        "a client stops on a condition that would have cleared: {class} needs only \
                         {} before the identical request succeeds",
                        class.change_required()
                    )
                } else {
                    format!(
                        "a client retries a request that will be refused identically forever; \
                         {class} requires a change to the {}",
                        class.change_required()
                    )
                },
                required_distinction: format!(
                    "give {class} a code whose is_retryable() is {truly_retryable}, or carry the \
                     retry decision in the JSON envelope rather than deriving it from the code"
                ),
            });
            continue;
        }

        if !code_meaning_covers(class) {
            divergences.push(Divergence {
                kind: DivergenceKind::MeaningNarrowerThanClass,
                severity: AuditSeverity::Imprecision,
                code,
                classes: vec![class],
                finding: format!(
                    "{class} maps to exit {code}, documented as {:?}, which is not what happened",
                    row.meaning
                ),
                consequence: format!(
                    "a reader of the exit code describes the run wrongly: {}",
                    class.gloss()
                ),
                required_distinction: format!(
                    "give {class} a code whose documented meaning is {:?}",
                    class.gloss()
                ),
            });
        }
    }

    for row in &shipped {
        let classes: Vec<DiagnosticClass> = DiagnosticClass::ALL
            .into_iter()
            .filter(|class| shipped_code_for(*class) == row.code)
            .collect();
        if classes.len() > 1 {
            let retry_decisions: Vec<Retryability> =
                classes.iter().map(|c| c.retryability()).collect();
            let loses_retry_decision = retry_decisions.windows(2).any(|w| w[0] != w[1]);
            divergences.push(Divergence {
                kind: DivergenceKind::ClassCollision,
                severity: if loses_retry_decision {
                    AuditSeverity::Defect
                } else {
                    AuditSeverity::Imprecision
                },
                code: row.code,
                classes: classes.clone(),
                finding: format!(
                    "exit {} ({}) carries {} classes: {}",
                    row.code,
                    row.slug,
                    classes.len(),
                    classes
                        .iter()
                        .map(|c| c.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                consequence: if loses_retry_decision {
                    "a script branching on the exit code cannot recover the retry decision, \
                     because the classes sharing this code do not agree on one"
                        .to_string()
                } else {
                    "the retry decision survives, but the operator-paging decision does not: the \
                     classes sharing this code disagree on is_system_failure()"
                        .to_string()
                },
                required_distinction: format!(
                    "split exit {} so that classes with different retry or paging decisions do not \
                     share a code",
                    row.code
                ),
            });
        }
        if classes.is_empty() && row.code != 0 {
            divergences.push(Divergence {
                kind: DivergenceKind::CodeOutsideTheTaxonomy,
                severity: AuditSeverity::Note,
                code: row.code,
                classes: Vec::new(),
                finding: format!(
                    "exit {} ({}) has no diagnostic class, because it reports a verdict rather \
                     than a failure: {}",
                    row.code, row.slug, row.meaning
                ),
                consequence: "none; 40.36 classifies failures and this code reports a completed \
                              run whose checked property did not hold"
                    .to_string(),
                required_distinction: "keep this code distinct from every failure code; a verdict \
                                       that shares a code with a failure is unscriptable"
                    .to_string(),
            });
        }
    }

    ExitCodeAudit {
        registry_source: SHIPPED_REGISTRY_SOURCE.to_string(),
        shipped_code_count: shipped.len(),
        class_count: DiagnosticClass::ALL.len(),
        divergences,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_registry_has_six_codes_and_only_code_five_advertises_a_retry() {
        let shipped = shipped_exit_codes();
        assert_eq!(shipped.len(), 6);
        let retryable: Vec<u8> = shipped
            .iter()
            .filter(|row| row.advertised_retryable)
            .map(|row| row.code)
            .collect();
        assert_eq!(retryable, vec![5]);
    }

    #[test]
    fn five_of_the_nine_classes_collapse_onto_exit_four() {
        let on_four: Vec<DiagnosticClass> = DiagnosticClass::ALL
            .into_iter()
            .filter(|c| shipped_code_for(*c) == 4)
            .collect();
        assert_eq!(on_four.len(), 5);
        assert!(on_four.contains(&DiagnosticClass::Stale));
        assert!(on_four.contains(&DiagnosticClass::Indeterminate));
    }

    #[test]
    fn stale_is_the_only_class_whose_advertised_retryability_is_inverted() {
        let inverted: Vec<DiagnosticClass> = audit()
            .divergences
            .iter()
            .filter(|d| d.kind == DivergenceKind::RetryabilityInverted)
            .flat_map(|d| d.classes.clone())
            .collect();
        assert_eq!(inverted, vec![DiagnosticClass::Stale]);
    }

    #[test]
    fn the_audit_finds_exactly_two_defects() {
        let audit = audit();
        let defects = audit.defects();
        assert_eq!(
            defects.len(),
            2,
            "unexpected defect set: {:?}",
            defects.iter().map(|d| &d.finding).collect::<Vec<_>>()
        );
        assert!(!audit.is_clean());
    }

    #[test]
    fn exit_one_is_reported_as_a_note_and_never_as_a_defect() {
        let audit = audit();
        let row = audit
            .divergences
            .iter()
            .find(|d| d.kind == DivergenceKind::CodeOutsideTheTaxonomy)
            .expect("exit 1 is reported");
        assert_eq!(row.code, 1);
        assert_eq!(row.severity, AuditSeverity::Note);
    }

    #[test]
    fn the_collision_on_exit_five_is_an_imprecision_because_both_classes_retry_alike() {
        let audit = audit();
        let row = audit
            .divergences
            .iter()
            .find(|d| d.kind == DivergenceKind::ClassCollision && d.code == 5)
            .expect("exit 5 collides");
        assert_eq!(row.severity, AuditSeverity::Imprecision);
        assert_eq!(
            row.classes,
            vec![DiagnosticClass::Unavailable, DiagnosticClass::Internal]
        );
    }

    #[test]
    fn no_class_contributes_two_rows_of_the_per_class_findings() {
        let audit = audit();
        let mut per_class: Vec<DiagnosticClass> = audit
            .divergences
            .iter()
            .filter(|d| {
                matches!(
                    d.kind,
                    DivergenceKind::RetryabilityInverted | DivergenceKind::MeaningNarrowerThanClass
                )
            })
            .flat_map(|d| d.classes.clone())
            .collect();
        let before = per_class.len();
        per_class.sort();
        per_class.dedup();
        assert_eq!(before, per_class.len());
    }

    #[test]
    fn the_audit_serialises_and_parses_back_unchanged() {
        let audit = audit();
        let encoded = serde_json::to_string(&audit).expect("serialises");
        let decoded: ExitCodeAudit = serde_json::from_str(&encoded).expect("parses back");
        assert_eq!(audit, decoded);
    }
}
