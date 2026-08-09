//! The lens grammar — blueprint 42.01.
//!
//! 42.01 asks for a way to "compile task-specific biological graph views instead of rendering one
//! universal graph hairball". Read as a UI instruction that is a layout problem. Read as an
//! epistemic instruction it is the more interesting one, and the one this crate takes: a lens is
//! not a view, it is a **declared question** together with the conditions under which it may be
//! answered.
//!
//! A [`LensDeclaration`] states four things, all of them before any evidence arrives:
//!
//! 1. the single question the lens answers, in words;
//! 2. the evidence it requires, named, so that absence is detectable rather than inferred;
//! 3. the scope dimensions that must be bound for the question to mean anything;
//! 4. the questions it **refuses**, with reasons, so a caller learns the boundary in advance.
//!
//! # Three outcomes, never two
//!
//! [`LensOutcome`] distinguishes an answer, a refusal, and absent evidence. Collapsing the last
//! two is the same mistake `bioprism-section` refuses when it keeps `InfluenceClass::Zero` apart
//! from `InfluenceClass::Unknown`: "this lens will not tell you" and "nobody gathered what it
//! would need" have different remedies, and a caller that cannot tell them apart will retry the
//! wrong one. A refusal is final and carries a declared reason. An evidence gap is repairable and
//! carries the requirements that were absent, each with a
//! [`Missingness`](crate::missingness::Missingness) class.
//!
//! # Completeness is a field, not a footnote
//!
//! 42.30 requires that a progressively rendered view never be mistaken for a finished one. Every
//! answered outcome therefore carries a [`Coverage`], whose fields are private and whose partial
//! constructor refuses to accept an empty list of pending regions. A partial answer must name
//! what it has not reached. [`Completeness`] is derived from the counts, never asserted, so there
//! is no code path that marks an unfinished sweep complete.
//!
//! # What is not here
//!
//! No renderer, no layout, no interaction model, no viewport, no deep links, no caching, no
//! streaming transport. 42.01 also requires a `PolicyDecision` on every view; access policy lives
//! in the policy fibers of 43.33 and is represented here only as
//! [`RefusalReason::PolicyWithheld`] and as
//! [`Missingness::PolicyWithheld`](crate::missingness::Missingness::PolicyWithheld) — this crate
//! records that a boundary was hit, it does not decide where the boundary is.

use crate::error::LensError;
use crate::missingness::Missingness;
use crate::nonvisual::{Witness, WitnessRow};
use bioprism_ids::ContentHash;
use bioprism_scope::ScopeKey;
use serde::{Deserialize, Serialize};

/// A stable identifier for a lens, used by the catalogue and the release gate.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LensId(String);

impl LensId {
    pub fn new(id: impl Into<String>) -> Self {
        LensId(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for LensId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A named piece of evidence a lens needs.
///
/// The `key` exists so that a gap can be reported as a fact about a specific input rather than as
/// a general shrug.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRequirement {
    pub key: String,
    pub description: String,
}

impl EvidenceRequirement {
    pub fn new(key: impl Into<String>, description: impl Into<String>) -> Self {
        EvidenceRequirement {
            key: key.into(),
            description: description.into(),
        }
    }
}

/// A scope dimension that must be bound before the lens's question is well posed.
///
/// "Does this biomarker predict response?" is not a question until someone says in which
/// population, on which assay, at which time. 43.03's typed scope base supplies the dimensions;
/// this type says which of them a given lens cannot proceed without.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopePrecondition {
    pub dimension: String,
    pub why: String,
}

impl ScopePrecondition {
    pub fn new(dimension: impl Into<String>, why: impl Into<String>) -> Self {
        ScopePrecondition {
            dimension: dimension.into(),
            why: why.into(),
        }
    }
}

/// Why a lens will not answer.
///
/// A closed set, because the catalogue is a contract and an open-ended reason field would let a
/// lens invent a refusal at answer time that no caller could have anticipated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefusalReason {
    /// The question asked is not the question this lens answers.
    OutOfScope,
    /// A declared scope dimension is unbound, so the question has no truth value yet.
    ScopePreconditionUnmet,
    /// Consent or access policy forbids the answer. The gap is real and stated.
    PolicyWithheld,
    /// The evidence is observational and the question demands an interventional claim. 42.12's
    /// reason for existing.
    WouldRequireInterventionalClaim,
    /// Answering would require pooling across scopes that do not refine one another, which
    /// manufactures a population that was never sampled.
    WouldAggregateIncomparableScopes,
    /// The question admits no answerable formulation over the evidence this lens accepts.
    NoAnswerableFormulation,
}

impl RefusalReason {
    pub fn as_str(self) -> &'static str {
        match self {
            RefusalReason::OutOfScope => "out_of_scope",
            RefusalReason::ScopePreconditionUnmet => "scope_precondition_unmet",
            RefusalReason::PolicyWithheld => "policy_withheld",
            RefusalReason::WouldRequireInterventionalClaim => "would_require_interventional_claim",
            RefusalReason::WouldAggregateIncomparableScopes => {
                "would_aggregate_incomparable_scopes"
            }
            RefusalReason::NoAnswerableFormulation => "no_answerable_formulation",
        }
    }
}

/// A refusal, with the concrete detail that makes it actionable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Refusal {
    pub reason: RefusalReason,
    pub detail: String,
}

impl Refusal {
    pub fn new(reason: RefusalReason, detail: impl Into<String>) -> Self {
        Refusal {
            reason,
            detail: detail.into(),
        }
    }
}

/// One required input that was not available, and why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AbsentRequirement {
    pub requirement: EvidenceRequirement,
    pub missingness: Missingness,
}

/// Evidence the lens needed and did not get.
///
/// Private field, one fallible constructor: an evidence gap that names nothing absent is a
/// refusal in disguise, and this crate refuses to let the two share a shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "EvidenceGapFields")]
pub struct EvidenceGap {
    absent: Vec<AbsentRequirement>,
}

#[derive(Deserialize)]
struct EvidenceGapFields {
    absent: Vec<AbsentRequirement>,
}

impl TryFrom<EvidenceGapFields> for EvidenceGap {
    type Error = LensError;

    fn try_from(fields: EvidenceGapFields) -> Result<Self, Self::Error> {
        EvidenceGap::new("<deserialized>", fields.absent)
    }
}

impl EvidenceGap {
    pub fn new(lens: &str, absent: Vec<AbsentRequirement>) -> Result<Self, LensError> {
        if absent.is_empty() {
            return Err(LensError::EmptyEvidenceGap {
                lens: lens.to_string(),
            });
        }
        Ok(EvidenceGap { absent })
    }

    pub fn absent(&self) -> &[AbsentRequirement] {
        &self.absent
    }

    /// True when every absent requirement is a measured absence or an explicit bound — that is,
    /// when the gap is itself informative rather than a hole.
    pub fn every_absence_was_measured(&self) -> bool {
        self.absent
            .iter()
            .all(|a| a.missingness.is_measured_result())
    }
}

/// A part of the eligible input a partial answer has not examined.
///
/// Named, not counted. "60% loaded" is the failure mode of 42.30; "strata `site=MGH` and
/// `site=DFCI` not yet examined" is a statement a reader can act on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingRegion {
    pub region: String,
    pub why: String,
}

impl PendingRegion {
    pub fn new(region: impl Into<String>, why: impl Into<String>) -> Self {
        PendingRegion {
            region: region.into(),
            why: why.into(),
        }
    }
}

/// Whether an answer covers everything it was eligible to cover.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "completeness", rename_all = "snake_case")]
pub enum Completeness {
    Complete,
    Partial { examined: usize, eligible: usize },
}

impl Completeness {
    pub fn is_complete(&self) -> bool {
        matches!(self, Completeness::Complete)
    }
}

/// How much of the eligible input an answer actually examined.
///
/// Private fields and no `Deserialize` that bypasses the check: [`Coverage::partial`] refuses an
/// empty pending list, and the deserializer routes through the same constructor. A `Coverage`
/// that claims to be partial while naming nothing outstanding does not exist at runtime, which is
/// the strongest available form of 42.30's requirement that "a partially loaded view must never
/// be indistinguishable from a complete one".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "CoverageFields")]
pub struct Coverage {
    examined: usize,
    eligible: usize,
    pending: Vec<PendingRegion>,
}

#[derive(Deserialize)]
struct CoverageFields {
    examined: usize,
    eligible: usize,
    #[serde(default)]
    pending: Vec<PendingRegion>,
}

impl TryFrom<CoverageFields> for Coverage {
    type Error = LensError;

    fn try_from(fields: CoverageFields) -> Result<Self, Self::Error> {
        if fields.pending.is_empty() {
            Coverage::complete("<deserialized>", fields.examined, fields.eligible)
        } else {
            Coverage::partial(
                "<deserialized>",
                fields.examined,
                fields.eligible,
                fields.pending,
            )
        }
    }
}

impl Coverage {
    /// The whole eligible input was examined.
    pub fn complete(lens: &str, examined: usize, eligible: usize) -> Result<Self, LensError> {
        if examined != eligible {
            return Err(LensError::IncoherentCoverage {
                lens: lens.to_string(),
                examined,
                eligible,
            });
        }
        Ok(Coverage {
            examined,
            eligible,
            pending: Vec::new(),
        })
    }

    /// Part of the eligible input was examined, and the rest is named.
    pub fn partial(
        lens: &str,
        examined: usize,
        eligible: usize,
        pending: Vec<PendingRegion>,
    ) -> Result<Self, LensError> {
        if pending.is_empty() {
            return Err(LensError::PartialCoverageWithoutPendingRegion {
                lens: lens.to_string(),
            });
        }
        if examined >= eligible {
            return Err(LensError::IncoherentCoverage {
                lens: lens.to_string(),
                examined,
                eligible,
            });
        }
        Ok(Coverage {
            examined,
            eligible,
            pending,
        })
    }

    pub fn examined(&self) -> usize {
        self.examined
    }

    pub fn eligible(&self) -> usize {
        self.eligible
    }

    pub fn pending(&self) -> &[PendingRegion] {
        &self.pending
    }

    /// Derived from the counts and the pending list. Never asserted by a caller.
    pub fn completeness(&self) -> Completeness {
        if self.pending.is_empty() && self.examined == self.eligible {
            Completeness::Complete
        } else {
            Completeness::Partial {
                examined: self.examined,
                eligible: self.eligible,
            }
        }
    }
}

/// What a lens produced.
///
/// Generic in the witness type so that the bound `W: Witness` — which is the 42.27 obligation —
/// is checked at the definition site of every lens, not at the point a report is built.
#[derive(Debug, Clone, PartialEq)]
pub enum LensOutcome<W> {
    /// The question was answered, with concrete witnesses and a stated coverage. An empty witness
    /// list is a legitimate answer ("no leakage found in what was examined") *only* because the
    /// coverage says what was examined.
    Answered {
        witnesses: Vec<W>,
        coverage: Coverage,
    },
    /// The lens will not answer, for a reason it declared in advance.
    Refused(Refusal),
    /// The lens would answer but the evidence it requires is absent.
    EvidenceAbsent(EvidenceGap),
}

/// A declared question over evidence.
///
/// The bound `type Witness: Witness` is the load-bearing line of this crate. `Witness` has no
/// method that yields a drawing, so an implementer must be able to state its findings as rows and
/// sentences. A "lens" that can only produce a rendering has no type to put here and does not
/// compile — 42.27 as a compile-time obligation rather than a review item.
pub trait Lens {
    /// Everything the lens reads. One type, so a caller can see the whole input surface.
    type Evidence;

    /// The finding shape. Must be answerable without vision.
    type Witness: Witness;

    /// The question, its requirements, its preconditions and its refusals. Constant for a lens.
    fn declaration(&self) -> LensDeclaration;

    /// Answer, refuse, or report the gap. Deterministic: no clock, no ambient state.
    fn answer(&self, scope: &ScopeKey, evidence: &Self::Evidence) -> LensOutcome<Self::Witness>;
}

/// What a lens says about itself before it sees any evidence.
///
/// Constructed through [`LensDeclaration::new`], which enforces the one consistency rule the
/// grammar can check statically: a lens that declares scope preconditions must also declare the
/// refusal it will issue when they are unmet. Declaring a precondition and then answering anyway
/// is worse than declaring none, because the declaration is what a caller trusts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LensDeclaration {
    id: LensId,
    blueprint_module: &'static str,
    question: String,
    requires: Vec<EvidenceRequirement>,
    preconditions: Vec<ScopePrecondition>,
    refuses: Vec<RefusalReason>,
}

impl LensDeclaration {
    pub fn new(
        id: LensId,
        blueprint_module: &'static str,
        question: impl Into<String>,
        requires: Vec<EvidenceRequirement>,
        preconditions: Vec<ScopePrecondition>,
        refuses: Vec<RefusalReason>,
    ) -> Result<Self, LensError> {
        let question = question.into();
        if question.trim().is_empty() {
            return Err(LensError::EmptyQuestion {
                lens: id.as_str().to_string(),
            });
        }
        if !preconditions.is_empty() && !refuses.contains(&RefusalReason::ScopePreconditionUnmet) {
            return Err(LensError::PreconditionWithoutRefusal {
                lens: id.as_str().to_string(),
                count: preconditions.len(),
            });
        }
        Ok(LensDeclaration {
            id,
            blueprint_module,
            question,
            requires,
            preconditions,
            refuses,
        })
    }

    pub fn id(&self) -> &LensId {
        &self.id
    }

    /// The blueprint module this lens implements, e.g. `"42.10"`.
    pub fn blueprint_module(&self) -> &'static str {
        self.blueprint_module
    }

    pub fn question(&self) -> &str {
        &self.question
    }

    pub fn requires(&self) -> &[EvidenceRequirement] {
        &self.requires
    }

    pub fn preconditions(&self) -> &[ScopePrecondition] {
        &self.preconditions
    }

    pub fn refuses(&self) -> &[RefusalReason] {
        &self.refuses
    }

    /// The scope dimensions this lens requires that `scope` does not bind.
    pub fn unmet_preconditions(&self, scope: &ScopeKey) -> Vec<&ScopePrecondition> {
        self.preconditions
            .iter()
            .filter(|p| scope.get(&p.dimension).is_none())
            .collect()
    }

    /// Whether this lens declared it might refuse for this reason.
    pub fn declares_refusal(&self, reason: RefusalReason) -> bool {
        self.refuses.contains(&reason)
    }
}

/// A lens outcome after type erasure, with a receipt.
///
/// `Serialize` only, with private fields and no public constructor. The single route to a
/// `LensReport` is [`run`], which checks the witnesses against their declared columns, checks
/// the refusal against the declaration, and computes the receipt by hashing the body. A report
/// cannot be assembled by hand, so a claim of "this lens answered" cannot be forged — the same
/// seal `bioprism-graph` puts on `View`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LensReport {
    lens: LensId,
    blueprint_module: &'static str,
    question: String,
    outcome: ReportOutcome,
    receipt: ContentHash,
}

/// The erased outcome carried by a [`LensReport`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ReportOutcome {
    Answered {
        witnesses: Vec<WitnessRow>,
        coverage: Coverage,
    },
    Refused(Refusal),
    EvidenceAbsent(EvidenceGap),
}

impl ReportOutcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            ReportOutcome::Answered { .. } => "answered",
            ReportOutcome::Refused(_) => "refused",
            ReportOutcome::EvidenceAbsent(_) => "evidence_absent",
        }
    }
}

impl LensReport {
    pub fn lens(&self) -> &LensId {
        &self.lens
    }

    pub fn blueprint_module(&self) -> &'static str {
        self.blueprint_module
    }

    pub fn question(&self) -> &str {
        &self.question
    }

    pub fn outcome(&self) -> &ReportOutcome {
        &self.outcome
    }

    /// A content hash over the canonical bytes of the report body. Deterministic and clock-free:
    /// the same lens over the same evidence yields the same receipt, in this process or another.
    pub fn receipt(&self) -> &ContentHash {
        &self.receipt
    }

    /// The witnesses, or an empty slice for a refusal or a gap.
    pub fn witnesses(&self) -> &[WitnessRow] {
        match &self.outcome {
            ReportOutcome::Answered { witnesses, .. } => witnesses,
            _ => &[],
        }
    }

    /// Completeness of the answer.
    ///
    /// A refusal and an evidence gap are complete *as statements*: the lens has said all it will
    /// say. Only an answer can be partial, and only through its [`Coverage`].
    pub fn completeness(&self) -> Completeness {
        match &self.outcome {
            ReportOutcome::Answered { coverage, .. } => coverage.completeness(),
            _ => Completeness::Complete,
        }
    }

    pub fn is_answered(&self) -> bool {
        matches!(self.outcome, ReportOutcome::Answered { .. })
    }

    /// The whole report as speakable lines: the question, the outcome, then one line per witness
    /// field. This is the 42.27 parity path, and it is the only rendition this crate emits.
    pub fn spoken(&self) -> Vec<String> {
        let mut lines = vec![format!("{}: {}", self.lens, self.question)];
        match &self.outcome {
            ReportOutcome::Answered {
                witnesses,
                coverage,
            } => {
                match coverage.completeness() {
                    Completeness::Complete => lines.push(format!(
                        "answered over all {} eligible unit(s)",
                        coverage.eligible()
                    )),
                    Completeness::Partial { examined, eligible } => {
                        lines.push(format!(
                            "answered over {examined} of {eligible} eligible unit(s); \
                             not yet examined:"
                        ));
                        for region in coverage.pending() {
                            lines.push(format!("  pending {}: {}", region.region, region.why));
                        }
                    }
                }
                if witnesses.is_empty() {
                    lines.push("no findings in what was examined".into());
                }
                for witness in witnesses {
                    lines.push(witness.sentence.clone());
                    for field in witness.spoken() {
                        lines.push(format!("  {field}"));
                    }
                }
            }
            ReportOutcome::Refused(refusal) => {
                lines.push(format!(
                    "refused ({}): {}",
                    refusal.reason.as_str(),
                    refusal.detail
                ));
            }
            ReportOutcome::EvidenceAbsent(gap) => {
                lines.push("not answered; required evidence absent:".into());
                for absent in gap.absent() {
                    lines.push(format!(
                        "  {}: {}",
                        absent.requirement.key,
                        absent.missingness.sentence()
                    ));
                }
            }
        }
        lines
    }
}

/// Run a lens and seal the result.
///
/// This is the only constructor of a [`LensReport`], and it enforces what the declaration
/// promised:
///
/// - unmet scope preconditions short-circuit to a refusal, so a lens never sees an ill-posed
///   question;
/// - a refusal for an undeclared reason is an error, not a report;
/// - a witness whose cells do not match its columns is an error, not a ragged table.
///
/// Deterministic. No clock, no ordering by hash-map iteration, no ambient configuration.
pub fn run<L: Lens>(
    lens: &L,
    scope: &ScopeKey,
    evidence: &L::Evidence,
) -> Result<LensReport, LensError> {
    let declaration = lens.declaration();
    let id = declaration.id().clone();

    let unmet = declaration.unmet_preconditions(scope);
    let outcome = if unmet.is_empty() {
        lens.answer(scope, evidence)
    } else {
        let dimensions: Vec<&str> = unmet.iter().map(|p| p.dimension.as_str()).collect();
        LensOutcome::Refused(Refusal::new(
            RefusalReason::ScopePreconditionUnmet,
            format!("unbound scope dimension(s): {}", dimensions.join(", ")),
        ))
    };

    let erased = match outcome {
        LensOutcome::Answered {
            witnesses,
            coverage,
        } => {
            let mut rows = Vec::with_capacity(witnesses.len());
            for witness in &witnesses {
                let row = WitnessRow::erase(witness).map_err(|(columns, cells)| {
                    LensError::MalformedWitness {
                        lens: id.as_str().to_string(),
                        kind: witness.kind(),
                        columns,
                        cells,
                    }
                })?;
                rows.push(row);
            }
            ReportOutcome::Answered {
                witnesses: rows,
                coverage,
            }
        }
        LensOutcome::Refused(refusal) => {
            if !declaration.declares_refusal(refusal.reason) {
                return Err(LensError::UndeclaredRefusal {
                    lens: id.as_str().to_string(),
                    reason: refusal.reason.as_str(),
                });
            }
            ReportOutcome::Refused(refusal)
        }
        LensOutcome::EvidenceAbsent(gap) => ReportOutcome::EvidenceAbsent(gap),
    };

    let body = serde_json::json!({
        "schema": crate::LENS_REPORT_SCHEMA_VERSION,
        "lens": id.as_str(),
        "blueprint_module": declaration.blueprint_module(),
        "question": declaration.question(),
        "outcome": erased,
    });
    let receipt = ContentHash::of_value(&body).map_err(|e| LensError::Uncanonicalisable {
        lens: id.as_str().to_string(),
        detail: e.to_string(),
    })?;

    Ok(LensReport {
        lens: id,
        blueprint_module: declaration.blueprint_module(),
        question: declaration.question().to_string(),
        outcome: erased,
        receipt,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::missingness::UnattemptedReason;
    use crate::nonvisual::Cell;

    struct Finding(&'static str);

    impl Witness for Finding {
        fn kind(&self) -> &'static str {
            "finding"
        }
        fn columns(&self) -> &'static [&'static str] {
            &["subject"]
        }
        fn cells(&self) -> Vec<Cell> {
            vec![Cell::id(self.0)]
        }
        fn sentence(&self) -> String {
            format!("subject {} flagged", self.0)
        }
    }

    struct RaggedFinding;

    impl Witness for RaggedFinding {
        fn kind(&self) -> &'static str {
            "ragged"
        }
        fn columns(&self) -> &'static [&'static str] {
            &["a", "b"]
        }
        fn cells(&self) -> Vec<Cell> {
            vec![Cell::text("only one")]
        }
        fn sentence(&self) -> String {
            "ragged".into()
        }
    }

    enum Behaviour {
        Answer,
        RefuseDeclared,
        RefuseUndeclared,
    }

    struct Toy {
        behaviour: Behaviour,
        preconditions: Vec<ScopePrecondition>,
        refuses: Vec<RefusalReason>,
    }

    impl Toy {
        fn answering() -> Self {
            Toy {
                behaviour: Behaviour::Answer,
                preconditions: Vec::new(),
                refuses: vec![RefusalReason::OutOfScope],
            }
        }
    }

    impl Lens for Toy {
        type Evidence = ();
        type Witness = Finding;

        fn declaration(&self) -> LensDeclaration {
            LensDeclaration::new(
                LensId::new("toy"),
                "42.01",
                "does the toy find anything?",
                vec![EvidenceRequirement::new("input", "the toy input")],
                self.preconditions.clone(),
                self.refuses.clone(),
            )
            .expect("toy declaration is well formed")
        }

        fn answer(&self, _scope: &ScopeKey, _evidence: &()) -> LensOutcome<Finding> {
            match self.behaviour {
                Behaviour::Answer => LensOutcome::Answered {
                    witnesses: vec![Finding("S001")],
                    coverage: Coverage::complete("toy", 3, 3).unwrap(),
                },
                Behaviour::RefuseDeclared => {
                    LensOutcome::Refused(Refusal::new(RefusalReason::OutOfScope, "wrong question"))
                }
                Behaviour::RefuseUndeclared => LensOutcome::Refused(Refusal::new(
                    RefusalReason::PolicyWithheld,
                    "never declared this",
                )),
            }
        }
    }

    struct RaggedLens;

    impl Lens for RaggedLens {
        type Evidence = ();
        type Witness = RaggedFinding;

        fn declaration(&self) -> LensDeclaration {
            LensDeclaration::new(
                LensId::new("ragged"),
                "42.01",
                "ragged?",
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
            .unwrap()
        }

        fn answer(&self, _scope: &ScopeKey, _evidence: &()) -> LensOutcome<RaggedFinding> {
            LensOutcome::Answered {
                witnesses: vec![RaggedFinding],
                coverage: Coverage::complete("ragged", 1, 1).unwrap(),
            }
        }
    }

    #[test]
    fn a_lens_declaring_a_scope_precondition_must_declare_the_refusal_it_will_issue() {
        let err = LensDeclaration::new(
            LensId::new("x"),
            "42.01",
            "q?",
            Vec::new(),
            vec![ScopePrecondition::new("subject", "needed")],
            vec![RefusalReason::OutOfScope],
        )
        .unwrap_err();
        assert!(matches!(err, LensError::PreconditionWithoutRefusal { .. }));
    }

    #[test]
    fn a_lens_with_no_question_is_not_a_lens() {
        let err = LensDeclaration::new(
            LensId::new("x"),
            "42.01",
            "   ",
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap_err();
        assert!(matches!(err, LensError::EmptyQuestion { .. }));
    }

    #[test]
    fn an_unbound_precondition_refuses_before_the_lens_sees_the_evidence() {
        let lens = Toy {
            behaviour: Behaviour::Answer,
            preconditions: vec![ScopePrecondition::new(
                "specimen",
                "the toy needs a specimen",
            )],
            refuses: vec![RefusalReason::ScopePreconditionUnmet],
        };
        let report = run(&lens, &ScopeKey::new(), &()).unwrap();
        match report.outcome() {
            ReportOutcome::Refused(refusal) => {
                assert_eq!(refusal.reason, RefusalReason::ScopePreconditionUnmet);
                assert!(refusal.detail.contains("specimen"));
            }
            other => panic!("expected refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_bound_precondition_lets_the_lens_answer() {
        let lens = Toy {
            behaviour: Behaviour::Answer,
            preconditions: vec![ScopePrecondition::new("specimen", "needed")],
            refuses: vec![RefusalReason::ScopePreconditionUnmet],
        };
        let scope = ScopeKey::new().exact("specimen", "SP-1");
        let report = run(&lens, &scope, &()).unwrap();
        assert!(report.is_answered());
    }

    #[test]
    fn a_lens_cannot_refuse_for_a_reason_it_never_declared() {
        let lens = Toy {
            behaviour: Behaviour::RefuseUndeclared,
            preconditions: Vec::new(),
            refuses: vec![RefusalReason::OutOfScope],
        };
        let err = run(&lens, &ScopeKey::new(), &()).unwrap_err();
        assert!(matches!(err, LensError::UndeclaredRefusal { .. }));
    }

    #[test]
    fn a_declared_refusal_is_reported_with_its_stated_reason() {
        let lens = Toy {
            behaviour: Behaviour::RefuseDeclared,
            preconditions: Vec::new(),
            refuses: vec![RefusalReason::OutOfScope],
        };
        let report = run(&lens, &ScopeKey::new(), &()).unwrap();
        assert_eq!(report.outcome().as_str(), "refused");
        assert!(report.spoken().iter().any(|l| l.contains("out_of_scope")));
    }

    #[test]
    fn a_witness_that_cannot_be_stated_as_a_row_is_rejected_rather_than_rendered() {
        let err = run(&RaggedLens, &ScopeKey::new(), &()).unwrap_err();
        assert!(matches!(
            err,
            LensError::MalformedWitness {
                columns: 2,
                cells: 1,
                ..
            }
        ));
    }

    #[test]
    fn a_partial_coverage_must_name_what_it_has_not_reached() {
        let err = Coverage::partial("toy", 4, 10, Vec::new()).unwrap_err();
        assert!(matches!(
            err,
            LensError::PartialCoverageWithoutPendingRegion { .. }
        ));
        let ok = Coverage::partial(
            "toy",
            4,
            10,
            vec![PendingRegion::new("site=MGH", "not yet loaded")],
        )
        .unwrap();
        assert_eq!(
            ok.completeness(),
            Completeness::Partial {
                examined: 4,
                eligible: 10
            }
        );
    }

    #[test]
    fn a_complete_coverage_cannot_be_claimed_over_an_unfinished_sweep() {
        let err = Coverage::complete("toy", 4, 10).unwrap_err();
        assert!(matches!(err, LensError::IncoherentCoverage { .. }));
    }

    #[test]
    fn a_partial_coverage_cannot_be_forged_through_deserialization() {
        let forged = r#"{"examined":4,"eligible":10,"pending":[]}"#;
        assert!(serde_json::from_str::<Coverage>(forged).is_err());
    }

    #[test]
    fn a_partial_answer_speaks_its_incompleteness_before_its_findings() {
        struct PartialLens;
        impl Lens for PartialLens {
            type Evidence = ();
            type Witness = Finding;
            fn declaration(&self) -> LensDeclaration {
                LensDeclaration::new(
                    LensId::new("partial"),
                    "42.30",
                    "partial?",
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                )
                .unwrap()
            }
            fn answer(&self, _s: &ScopeKey, _e: &()) -> LensOutcome<Finding> {
                LensOutcome::Answered {
                    witnesses: vec![Finding("S001")],
                    coverage: Coverage::partial(
                        "partial",
                        4,
                        10,
                        vec![PendingRegion::new("site=DFCI", "shard not loaded")],
                    )
                    .unwrap(),
                }
            }
        }
        let report = run(&PartialLens, &ScopeKey::new(), &()).unwrap();
        assert!(!report.completeness().is_complete());
        let spoken = report.spoken();
        assert!(spoken.iter().any(|l| l.contains("4 of 10")));
        assert!(spoken.iter().any(|l| l.contains("site=DFCI")));
    }

    #[test]
    fn an_evidence_gap_that_names_nothing_absent_is_not_constructible() {
        let err = EvidenceGap::new("toy", Vec::new()).unwrap_err();
        assert!(matches!(err, LensError::EmptyEvidenceGap { .. }));
    }

    #[test]
    fn absent_evidence_is_not_a_refusal() {
        let gap = EvidenceGap::new(
            "toy",
            vec![AbsentRequirement {
                requirement: EvidenceRequirement::new("site", "site of each subject"),
                missingness: Missingness::NeverMeasured {
                    reason: UnattemptedReason::Unrecorded,
                },
            }],
        )
        .unwrap();
        let refusal = Refusal::new(RefusalReason::OutOfScope, "nope");
        let a = ReportOutcome::EvidenceAbsent(gap);
        let b = ReportOutcome::Refused(refusal);
        assert_ne!(a.as_str(), b.as_str());
        assert_ne!(a, b);
    }

    #[test]
    fn the_receipt_is_deterministic_across_runs() {
        let lens = Toy::answering();
        let first = run(&lens, &ScopeKey::new(), &()).unwrap();
        let second = run(&lens, &ScopeKey::new(), &()).unwrap();
        assert_eq!(first.receipt(), second.receipt());
        assert_eq!(first.receipt().as_str().len(), 64);
    }

    #[test]
    fn a_different_outcome_gets_a_different_receipt() {
        let answering = run(&Toy::answering(), &ScopeKey::new(), &()).unwrap();
        let refusing = run(
            &Toy {
                behaviour: Behaviour::RefuseDeclared,
                preconditions: Vec::new(),
                refuses: vec![RefusalReason::OutOfScope],
            },
            &ScopeKey::new(),
            &(),
        )
        .unwrap();
        assert_ne!(answering.receipt(), refusing.receipt());
    }

    #[test]
    fn an_empty_witness_list_is_an_answer_only_alongside_its_coverage() {
        struct Clean;
        impl Lens for Clean {
            type Evidence = ();
            type Witness = Finding;
            fn declaration(&self) -> LensDeclaration {
                LensDeclaration::new(
                    LensId::new("clean"),
                    "42.01",
                    "anything?",
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                )
                .unwrap()
            }
            fn answer(&self, _s: &ScopeKey, _e: &()) -> LensOutcome<Finding> {
                LensOutcome::Answered {
                    witnesses: Vec::new(),
                    coverage: Coverage::complete("clean", 12, 12).unwrap(),
                }
            }
        }
        let report = run(&Clean, &ScopeKey::new(), &()).unwrap();
        assert!(report.witnesses().is_empty());
        assert!(report
            .spoken()
            .iter()
            .any(|l| l.contains("all 12 eligible")));
    }
}
