//! The exit-code audit: what `bioprism-cli` ships versus what the taxonomy needs.
//!
//! Blueprint 11.03 (CLI specification) and 40.13 both require errors to map to documented exit
//! codes, and 40.36 requires the retry classification to survive that mapping. This module holds
//! the shipped registry against that requirement and says exactly where it fails and what it
//! costs.
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
//! **Both defects are fixed.** The registry now ships ten codes, and every failure code carries
//! exactly one [`Retryability`]:
//!
//! - **Exit 4 no longer carries five classes.** `Conflict`, `PolicyDenied` and `Indeterminate` were
//!   split out to codes 6, 7 and 8, and `Stale` to 9, leaving 4 to mean `ContractViolation` and
//!   nothing else. A CI script can now tell a policy refusal from an oracle abstention from a
//!   snapshot that moved under it, which is what those three different next actions require.
//! - **`Stale` is no longer advertised as terminal.** Exit 9's `is_retryable()` is `true`, matching
//!   [`Retryability::RetryableAsIs`]. This was the only place in the registry where the advertised
//!   retry decision was the *opposite* of the true one rather than merely coarser, and it is the
//!   reason `Stale` got a code of its own rather than being folded in beside `Unavailable`.
//!
//! Both were first named by `bioprism-services`, reproduced here from an independent table, and
//! fixed in `bioprism-cli`. The rows that reported them are not deleted — they are computed from
//! the two tables below and simply no longer fire, which is the only form of "fixed" this module
//! can honestly report. [`registry_before_the_split`] retains the registry that *did* fire them, so
//! the detector is still exercised against a known positive rather than only against a clean input.
//!
//! **Two imprecision rows are left, and they are two views of one fact.** `Internal` shares exit 5
//! with `Unavailable`, which the audit reports once as a collision on the code and once as a
//! meaning too narrow for the class — a fault of this binary described as a dependency that could
//! not be read. The two classes agree on the retry decision and on the paging decision, so nothing
//! a caller acts on is lost; what is lost is the ability to say which of them happened. Giving
//! `Internal` a code of its own is a judgement about whether an unclassified fault of the binary
//! deserves to be distinguishable at the process boundary, and this module reports it rather than
//! deciding it.
//!
//! # What is deliberately not claimed
//!
//! Exit code 1, `AssertionFailed`, has no class in the taxonomy, and this module reports that as a
//! **note, not a defect**. 40.36 classifies failures; exit 1 is a completed run whose checked
//! property did not hold, which is a verdict. Filing it as a taxonomy gap would be a fabricated
//! finding, and an audit that inflates its own count is one nobody reads twice.

use crate::diagnostic::{Certainty, Diagnostic, DiagnosticCode, Remedy, Site};
use crate::taxonomy::{ChangeRequired, DiagnosticClass, Retryability};
use serde::{Deserialize, Serialize};

/// The file a reviewer opens to check the transcription below.
pub const SHIPPED_REGISTRY_SOURCE: &str = "crates/cli/src/exit.rs";

/// The fields copied out of that file. Anything else about the CLI is not modelled here.
pub const SHIPPED_REGISTRY_TRANSCRIBED_FIELDS: [&str; 5] = [
    "ExitCode discriminant",
    "ExitCode::slug",
    "ExitCode::summary",
    "ExitCode::is_retryable",
    "ExitCode::retryability",
];

/// One row of an exit-code registry, transcribed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShippedExitCode {
    pub code: u8,
    /// `ExitCode::slug`.
    pub slug: String,
    /// `ExitCode::summary`: the line `bioprism --help` prints for the code.
    ///
    /// Deliberately the string a user reads rather than the doc comment above it, because
    /// [`code_meaning_covers`] asks whether somebody who has only seen the code's stated meaning
    /// would describe the run correctly, and what they will have seen is `--help`.
    /// [`registry_before_the_split`] predates that accessor and transcribes doc comments instead.
    pub meaning: String,
    /// `ExitCode::is_retryable`, which is `true` for codes 5 and 9.
    pub advertised_retryable: bool,
    /// `ExitCode::retryability`, the three-valued decision the code publishes.
    ///
    /// `None` means the registry publishes no decision for this code. For codes 0 and 1 that is
    /// correct and intended — they report a verdict, not a failure. A registry with no such
    /// accessor at all, as [`registry_before_the_split`] had none, is `None` throughout, and
    /// [`audit_registry`] falls back to the boolean for it rather than counting every code as a
    /// finding; the boolean is what that registry actually published and it is what its consumers
    /// actually read.
    pub advertised_retryability: Option<Retryability>,
}

/// The ten codes `bioprism-cli` ships.
pub fn shipped_exit_codes() -> Vec<ShippedExitCode> {
    let terminal = Some(Retryability::Terminal);
    let after_change = Some(Retryability::RetryableAfterChange);
    let as_is = Some(Retryability::RetryableAsIs);
    let rows: [(u8, &str, &str, bool, Option<Retryability>); 10] = [
        (
            0,
            "ok",
            "the command completed and its assertion held",
            false,
            None,
        ),
        (
            1,
            "assertion_failed",
            "completed, but the checked property does not hold",
            false,
            None,
        ),
        (2, "usage", "bad invocation", false, terminal),
        (
            3,
            "invalid_input",
            "input failed its schema or could not be parsed",
            false,
            terminal,
        ),
        (
            4,
            "compile_failed",
            "no result satisfies the declared contract",
            false,
            after_change,
        ),
        (
            5,
            "io",
            "a declared dependency could not be read or written",
            true,
            as_is,
        ),
        (
            6,
            "conflict",
            "contradicts state already committed under this id",
            false,
            terminal,
        ),
        (
            7,
            "policy_denied",
            "policy refused; the platform behaved correctly",
            false,
            after_change,
        ),
        (
            8,
            "indeterminate",
            "ran correctly; the evidence does not decide",
            false,
            after_change,
        ),
        (
            9,
            "stale",
            "a precondition was superseded; re-read and re-send",
            true,
            as_is,
        ),
    ];
    rows.into_iter()
        .map(
            |(code, slug, meaning, retryable, retryability)| ShippedExitCode {
                code,
                slug: slug.to_string(),
                meaning: meaning.to_string(),
                advertised_retryable: retryable,
                advertised_retryability: retryability,
            },
        )
        .collect()
}

/// The code `bioprism-cli` returns for a diagnostic class.
///
/// Eight failure codes for nine classes, so exactly one code carries two: `Unavailable` and
/// `Internal` share 5. They agree on the retry decision, which is why the sharing is survivable and
/// why this table does not force a tenth code to make the count come out even.
pub fn shipped_code_for(class: DiagnosticClass) -> u8 {
    match class {
        DiagnosticClass::Usage => 2,
        DiagnosticClass::InvalidInput => 3,
        DiagnosticClass::ContractViolation => 4,
        DiagnosticClass::Unavailable | DiagnosticClass::Internal => 5,
        DiagnosticClass::Conflict => 6,
        DiagnosticClass::PolicyDenied => 7,
        DiagnosticClass::Indeterminate => 8,
        DiagnosticClass::Stale => 9,
    }
}

/// Whether the code's stated meaning actually covers the class, or merely absorbs it.
///
/// This is the judgement in the module, and it is written where a reader can disagree with it. The
/// question asked of each pair is narrow: *would a developer reading only the code's doc comment
/// correctly describe what happened?* Eight of the nine now answer yes, because eight classes have
/// a code whose documented meaning is theirs alone.
///
/// `Internal` is the one that does not. An unclassified fault of the binary reported as "a declared
/// dependency could not be read or written" sends the reader to look at the filesystem for a bug in
/// the tool.
pub fn code_meaning_covers(class: DiagnosticClass) -> bool {
    match class {
        DiagnosticClass::Usage
        | DiagnosticClass::InvalidInput
        | DiagnosticClass::Stale
        | DiagnosticClass::Conflict
        | DiagnosticClass::PolicyDenied
        | DiagnosticClass::ContractViolation
        | DiagnosticClass::Indeterminate
        | DiagnosticClass::Unavailable => true,
        DiagnosticClass::Internal => false,
    }
}

/// Where one class lands, and whether the code it lands on says what happened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassRouting {
    pub class: DiagnosticClass,
    pub code: u8,
    pub meaning_covers: bool,
}

/// A registry and its class routing, as one auditable value.
///
/// [`audit_registry`] takes this rather than reading the shipped tables directly, so that the
/// detector can be pointed at a registry the workspace does not ship. Without that, every
/// assertion about the audit after a clean result is an assertion that a function returned an empty
/// list, which a function that had stopped working would also satisfy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryUnderAudit {
    /// The document a reviewer opens to check the rows.
    pub source: String,
    pub codes: Vec<ShippedExitCode>,
    pub classification: Vec<ClassRouting>,
}

/// The registry `bioprism-cli` ships today, assembled from the two tables above.
pub fn shipped_registry() -> RegistryUnderAudit {
    RegistryUnderAudit {
        source: SHIPPED_REGISTRY_SOURCE.to_string(),
        codes: shipped_exit_codes(),
        classification: DiagnosticClass::ALL
            .into_iter()
            .map(|class| ClassRouting {
                class,
                code: shipped_code_for(class),
                meaning_covers: code_meaning_covers(class),
            })
            .collect(),
    }
}

/// The registry as it shipped before exit 4 was split, retained as the detector's known positive.
///
/// Six codes, of which four carried failures, and no three-valued accessor at all — every row's
/// `advertised_retryability` is `None` because `ExitCode` had only `is_retryable`. Auditing it must
/// still produce the two defects `bioprism-services` first named, and a test says so. Keeping the
/// pre-fix registry is what separates *the defect is fixed* from *the detector stopped looking*;
/// deleting it would leave the clean result on the shipped registry unfalsifiable.
pub fn registry_before_the_split() -> RegistryUnderAudit {
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
    let collapsed = |class: DiagnosticClass| match class {
        DiagnosticClass::Usage => 2,
        DiagnosticClass::InvalidInput => 3,
        DiagnosticClass::Stale
        | DiagnosticClass::Conflict
        | DiagnosticClass::PolicyDenied
        | DiagnosticClass::ContractViolation
        | DiagnosticClass::Indeterminate => 4,
        DiagnosticClass::Unavailable | DiagnosticClass::Internal => 5,
    };
    let covered = |class: DiagnosticClass| {
        matches!(
            class,
            DiagnosticClass::Usage
                | DiagnosticClass::InvalidInput
                | DiagnosticClass::ContractViolation
                | DiagnosticClass::Unavailable
        )
    };
    RegistryUnderAudit {
        source: format!("{SHIPPED_REGISTRY_SOURCE}@before-the-split"),
        codes: rows
            .into_iter()
            .map(|(code, slug, meaning, retryable)| ShippedExitCode {
                code,
                slug: slug.to_string(),
                meaning: meaning.to_string(),
                advertised_retryable: retryable,
                advertised_retryability: None,
            })
            .collect(),
        classification: DiagnosticClass::ALL
            .into_iter()
            .map(|class| ClassRouting {
                class,
                code: collapsed(class),
                meaning_covers: covered(class),
            })
            .collect(),
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
    /// Whether every failure code carries exactly one retry decision.
    ///
    /// The property the split was for, recorded as a field rather than left to be re-derived by
    /// each reader of [`ExitCodeAudit::divergences`]. It is the audit's headline: a caller holding
    /// nothing but the process status can decide whether to re-send.
    pub retry_decision_recoverable_from_the_code_alone: bool,
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
            .map(|(index, divergence)| {
                divergence_to_diagnostic(index, divergence, &self.registry_source)
            })
            .collect()
    }
}

fn divergence_to_diagnostic(index: usize, divergence: &Divergence, source: &str) -> Diagnostic {
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
    let site = || Site::Source {
        document: source.to_string(),
        span: None,
    };
    let mut diagnostic = Diagnostic::new(
        code,
        class,
        format!(
            "an exit code preserves the retry decision and the class distinction of the failure it reports ({})",
            divergence.kind.as_str()
        ),
        divergence.finding.clone(),
        site(),
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
        site(),
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

/// Run the audit against the shipped registry.
pub fn audit() -> ExitCodeAudit {
    audit_registry(&shipped_registry())
}

/// Run the audit against any registry.
///
/// Derived from the registry's own rows rather than written out, so a change to either table
/// changes the report and a synthetic registry is audited by exactly the code that audits the
/// shipped one. Each class contributes at most one row, taking the worst finding, because counting
/// a class twice would inflate the report without adding a fact.
pub fn audit_registry(registry: &RegistryUnderAudit) -> ExitCodeAudit {
    let mut divergences = Vec::new();

    for routing in &registry.classification {
        let class = routing.class;
        let code = routing.code;
        let row = registry
            .codes
            .iter()
            .find(|row| row.code == code)
            .expect("every routed code is in the registry");
        let truly = class.retryability();
        let advertised_permits_retry = row.advertised_retryable;
        let boolean_inverted = advertised_permits_retry != truly.permits_automatic_retry();
        let decision_disagrees = matches!(row.advertised_retryability, Some(a) if a != truly);

        if boolean_inverted || decision_disagrees {
            divergences.push(Divergence {
                kind: DivergenceKind::RetryabilityInverted,
                severity: AuditSeverity::Defect,
                code,
                classes: vec![class],
                finding: match row.advertised_retryability {
                    Some(advertised) => format!(
                        "{class} maps to exit {code} ({}), which advertises {advertised}, but \
                         {class} is {truly}",
                        row.slug
                    ),
                    None => format!(
                        "{class} maps to exit {code} ({}), which publishes no retry decision and \
                         whose is_retryable() is {advertised_permits_retry}, but {class} is {truly}",
                        row.slug
                    ),
                },
                consequence: if truly.permits_automatic_retry() {
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
                    "give {class} a code that publishes {truly}, or carry the retry decision in \
                     the JSON envelope rather than deriving it from the code"
                ),
            });
            continue;
        }

        if !routing.meaning_covers {
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

    for row in &registry.codes {
        let classes: Vec<DiagnosticClass> = registry
            .classification
            .iter()
            .filter(|routing| routing.code == row.code)
            .map(|routing| routing.class)
            .collect();
        if classes.len() > 1 {
            let loses_retry_decision = classes
                .windows(2)
                .any(|w| w[0].retryability() != w[1].retryability());
            let loses_paging_decision = classes
                .windows(2)
                .any(|w| w[0].is_system_failure() != w[1].is_system_failure());
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
                } else if loses_paging_decision {
                    "the retry decision survives, but the operator-paging decision does not: the \
                     classes sharing this code disagree on is_system_failure()"
                        .to_string()
                } else {
                    "the retry decision and the paging decision both survive; what is lost is \
                     which of the classes occurred, so a failure report cannot attribute the run"
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
        registry_source: registry.source.clone(),
        shipped_code_count: registry.codes.len(),
        class_count: registry.classification.len(),
        retry_decision_recoverable_from_the_code_alone: retry_decision_is_recoverable(registry),
        divergences,
    }
}

/// Whether every code in the registry carries exactly one retry decision.
///
/// The property is over *codes*, not over classes: a caller holding a process status has the code
/// and nothing else, so two classes sharing a code are indistinguishable to it. The decision
/// survives the sharing only when the classes sharing it agree.
pub fn retry_decision_is_recoverable(registry: &RegistryUnderAudit) -> bool {
    registry.codes.iter().all(|row| {
        let mut decisions = registry
            .classification
            .iter()
            .filter(|routing| routing.code == row.code)
            .map(|routing| routing.class.retryability());
        let Some(first) = decisions.next() else {
            return true;
        };
        decisions.all(|decision| decision == first)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_registry_has_ten_codes_and_the_two_retryable_ones_are_io_and_stale() {
        let shipped = shipped_exit_codes();
        assert_eq!(shipped.len(), 10);
        let retryable: Vec<u8> = shipped
            .iter()
            .filter(|row| row.advertised_retryable)
            .map(|row| row.code)
            .collect();
        assert_eq!(retryable, vec![5, 9]);
    }

    #[test]
    fn no_two_classes_that_disagree_on_retryability_share_a_shipped_code() {
        assert!(retry_decision_is_recoverable(&shipped_registry()));
        assert!(audit().retry_decision_recoverable_from_the_code_alone);
    }

    #[test]
    fn exit_four_carries_one_class_where_it_used_to_carry_five() {
        let on_four: Vec<DiagnosticClass> = DiagnosticClass::ALL
            .into_iter()
            .filter(|c| shipped_code_for(*c) == 4)
            .collect();
        assert_eq!(on_four, vec![DiagnosticClass::ContractViolation]);

        let was_on_four: Vec<DiagnosticClass> = registry_before_the_split()
            .classification
            .into_iter()
            .filter(|routing| routing.code == 4)
            .map(|routing| routing.class)
            .collect();
        assert_eq!(was_on_four.len(), 5);
    }

    #[test]
    fn stale_has_a_code_of_its_own_and_that_code_advertises_a_retry() {
        assert_eq!(shipped_code_for(DiagnosticClass::Stale), 9);
        let row = shipped_exit_codes()
            .into_iter()
            .find(|row| row.code == 9)
            .expect("exit 9 is in the registry");
        assert!(row.advertised_retryable);
        assert_eq!(
            row.advertised_retryability,
            Some(DiagnosticClass::Stale.retryability())
        );
    }

    #[test]
    fn the_audit_finds_no_defects_on_the_shipped_registry() {
        let audit = audit();
        assert!(
            audit.is_clean(),
            "unexpected defect set: {:?}",
            audit
                .defects()
                .iter()
                .map(|d| &d.finding)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn the_audit_still_finds_both_defects_in_the_registry_that_had_them() {
        let audit = audit_registry(&registry_before_the_split());
        assert_eq!(audit.defects().len(), 2);
        assert!(!audit.retry_decision_recoverable_from_the_code_alone);
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
        for registry in [shipped_registry(), registry_before_the_split()] {
            let mut per_class: Vec<DiagnosticClass> = audit_registry(&registry)
                .divergences
                .iter()
                .filter(|d| {
                    matches!(
                        d.kind,
                        DivergenceKind::RetryabilityInverted
                            | DivergenceKind::MeaningNarrowerThanClass
                    )
                })
                .flat_map(|d| d.classes.clone())
                .collect();
            let before = per_class.len();
            per_class.sort();
            per_class.dedup();
            assert_eq!(before, per_class.len());
        }
    }

    #[test]
    fn the_audit_serialises_and_parses_back_unchanged() {
        let audit = audit();
        let encoded = serde_json::to_string(&audit).expect("serialises");
        let decoded: ExitCodeAudit = serde_json::from_str(&encoded).expect("parses back");
        assert_eq!(audit, decoded);
    }
}
