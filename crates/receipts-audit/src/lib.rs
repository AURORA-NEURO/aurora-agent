//! An adversarial mutation battery for the workspace's digest-sealed receipts.
//!
//! Every receipt this workspace emits — the context certificate, the autopilot report, the
//! research dossier, the mission evidence bundle, the delivery receipt — makes the same claim:
//! *recompute the digest and you will catch any later edit*. A handful of happy-path tests plus
//! one hand-written tamper case does not establish that claim. It establishes that the verifier
//! catches the one edit whoever wrote the test happened to think of.
//!
//! This crate exists to replace that with a measurement. It enumerates every position in a
//! well-formed document, generates a structure-aware mutation at each one, states in advance
//! whether that mutation is formatting-only or semantic, and checks the verifier against the
//! statement. The result is a count: how many positions, how many cases, and — the number that
//! matters — how many cases the verifier got wrong.
//!
//! # Refused is not enough
//!
//! A battery that only asked *was this refused?* would pass a verifier that answers every question
//! with the same wrong word. The classes in [`RejectionClass`] are a shipped distinction: a
//! digest that is the wrong *shape* is a defect in the claimed digest, and a digest that is the
//! right shape and the wrong *value* is evidence that the body moved. Reporting the first as the
//! second accuses a caller of tampering on the strength of a typo — a defect this battery has
//! already found once in this workspace.
//!
//! So where the correct class is determined, the battery demands it. Tell a [`BatteryConfig`]
//! which field seals the document, and [`refine`] turns each generator's bare *refused* into the
//! specific answer that case has to produce: a well-formed digest that no longer matches its body
//! is a `digest_mismatch` and nothing else, a digest of the wrong shape is `digest_malformed`, a
//! digest that is gone is `digest_absent`. An edit that moved the body and left the claimed digest
//! exactly as issued may be reported as a mismatch, a structural failure, or a document the
//! verifier cannot read — but never as a defect in a digest that is intact.
//!
//! # What the library gives a test
//!
//! - [`rng::SplitMix64`], so a reported failure carries a seed that regenerates it exactly.
//! - [`walk`], which enumerates every JSON pointer in a document and patches at one.
//! - [`mutators`], the generators, each declaring what it produces.
//! - [`run_cases`], which feeds a batch of generated cases to a verifier without ever copying the
//!   document, and [`run_battery`], which generates the batch first and reports [`Coverage`]
//!   alongside any [`Hole`].
//!
//! # What a green battery does and does not mean
//!
//! A green run means: for this document, at these positions, with these generators, the verifier
//! answered the way the generator said it must. It is a statement about the verifier, not about
//! SHA-256 and not about the code that produced the document. The full list of things the battery
//! does not prove is in `docs/RECEIPTS_AUDIT.md`, and it belongs next to any citation of these
//! numbers.

pub mod mutators;
pub mod rng;
pub mod walk;

use bioprism_ids::ContentHash;
use mutators::Mutation;
use rng::SplitMix64;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;

pub use mutators::Mutation as GeneratedCase;

/// Why a verifier refused a document.
///
/// The classes are separate because the distinction between them is a shipped feature, not an
/// implementation detail: a digest that is the wrong *shape* is a defect in the claimed digest,
/// and a digest that is the right shape and the wrong *value* is evidence that the body moved.
/// Reporting the first as the second would accuse a caller of tampering on the strength of a typo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RejectionClass {
    /// The claimed digest is well formed and does not match the recomputed one.
    DigestMismatch,
    /// The claimed digest is present but is not a 64-character lowercase hex string.
    DigestMalformed,
    /// The document carries no digest field at all.
    DigestAbsent,
    /// The document does not have the shape the verifier requires.
    Malformed,
    /// The digest verifies and some other contract the verifier checks does not.
    StructuralFailure,
}

impl RejectionClass {
    pub fn as_str(self) -> &'static str {
        match self {
            RejectionClass::DigestMismatch => "digest_mismatch",
            RejectionClass::DigestMalformed => "digest_malformed",
            RejectionClass::DigestAbsent => "digest_absent",
            RejectionClass::Malformed => "malformed",
            RejectionClass::StructuralFailure => "structural_failure",
        }
    }
}

impl fmt::Display for RejectionClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Every class a refusal may carry, for a case the battery knows nothing else about.
pub const ANY_CLASS: &[RejectionClass] = &[
    RejectionClass::DigestMismatch,
    RejectionClass::DigestMalformed,
    RejectionClass::DigestAbsent,
    RejectionClass::Malformed,
    RejectionClass::StructuralFailure,
];

/// The classes a verifier may answer with for an edit that moved the body and left the claimed
/// digest present and exactly as issued.
///
/// `digest_absent` and `digest_malformed` are excluded on purpose. Both say the claimed digest is
/// defective, and it is not: it is the digest the producer issued, sitting untouched in the field
/// the producer put it in. A verifier that answers either of those to a body edit is naming the
/// wrong party.
pub const BODY_EDIT_CLASSES: &[RejectionClass] = &[
    RejectionClass::DigestMismatch,
    RejectionClass::Malformed,
    RejectionClass::StructuralFailure,
];

/// A claimed digest that is present but empty, or present and not a string at all.
const ABSENT_OR_MALFORMED: &[RejectionClass] = &[
    RejectionClass::DigestAbsent,
    RejectionClass::DigestMalformed,
];

/// A claimed digest of the wrong shape, in a document whose body moved in the same edit.
const MALFORMED_OR_UNREADABLE: &[RejectionClass] =
    &[RejectionClass::DigestMalformed, RejectionClass::Malformed];

/// A claimed digest that went missing in an edit that also moved the body.
const ABSENT_OR_UNREADABLE: &[RejectionClass] =
    &[RejectionClass::DigestAbsent, RejectionClass::Malformed];

/// Which refusals a case will accept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Refused, with no claim about the class. A generator produces this; [`refine`] replaces it
    /// with something sharper wherever the document says what the right answer is.
    Any,
    /// This class and no other.
    Exactly(RejectionClass),
    /// Any one of these, and nothing outside them.
    OneOf(&'static [RejectionClass]),
}

impl Refusal {
    pub fn permits(self, class: RejectionClass) -> bool {
        match self {
            Refusal::Any => true,
            Refusal::Exactly(expected) => expected == class,
            Refusal::OneOf(classes) => classes.contains(&class),
        }
    }

    /// Whether this refusal names one class and one only.
    pub fn is_pinned(self) -> bool {
        matches!(self, Refusal::Exactly(_))
            || matches!(self, Refusal::OneOf(classes) if classes.len() == 1)
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::Any => f.write_str("rejected"),
            Refusal::Exactly(class) => write!(f, "rejected as {class}"),
            Refusal::OneOf(classes) => {
                let names: Vec<&str> = classes.iter().map(|class| class.as_str()).collect();
                write!(f, "rejected as one of [{}]", names.join(", "))
            }
        }
    }
}

/// What a mutation claims the verifier must do with it.
///
/// The two variants are the whole point of the battery, and there is deliberately no third: a
/// formatting-only edit leaves the canonical bytes alone and so must leave the verdict alone, and a
/// semantic edit moves them and so must be refused. A generator that could not decide which of the
/// two it was producing would be testing nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expect {
    /// The canonical bytes are unchanged and so is the verdict, whatever that verdict was.
    VerdictUnchanged,
    /// The canonical bytes differ and the document must be refused, with the class this names.
    Rejected(Refusal),
}

impl fmt::Display for Expect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expect::VerdictUnchanged => f.write_str("verdict_unchanged"),
            Expect::Rejected(refusal) => write!(f, "{refusal}"),
        }
    }
}

/// One verifier's answer, projected to what the battery needs to judge it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Accepted,
    Rejected {
        class: RejectionClass,
        detail: String,
    },
}

impl Verdict {
    pub fn rejected(class: RejectionClass, detail: impl Into<String>) -> Self {
        Verdict::Rejected {
            class,
            detail: detail.into(),
        }
    }

    pub fn is_accepted(&self) -> bool {
        matches!(self, Verdict::Accepted)
    }

    pub fn class(&self) -> Option<RejectionClass> {
        match self {
            Verdict::Accepted => None,
            Verdict::Rejected { class, .. } => Some(*class),
        }
    }

    /// The name this verdict is counted under in [`Coverage::outcomes`].
    pub fn outcome(&self) -> &'static str {
        match self {
            Verdict::Accepted => "accepted",
            Verdict::Rejected { class, .. } => class.as_str(),
        }
    }
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Verdict::Accepted => f.write_str("accepted"),
            Verdict::Rejected { class, detail } => {
                write!(f, "rejected ({}): {detail}", class.as_str())
            }
        }
    }
}

/// How much of a document one battery run visited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Coverage {
    pub label: &'static str,
    pub seed: u64,
    /// Every JSON pointer the document contains.
    pub positions_total: usize,
    /// The pointers the structural families were applied at.
    pub positions_covered: usize,
    /// `1` when coverage is exhaustive; otherwise every `position_step`-th pointer was visited.
    pub position_step: usize,
    /// Digest fields found by shape. Always covered in full, whatever the position budget is.
    pub digest_fields: usize,
    /// Single-character digest mutations attempted, over all digest fields and all offsets.
    pub digest_offsets_covered: usize,
    pub cases: usize,
    pub cases_by_mutator: Vec<(&'static str, usize)>,
    /// Cases whose refusal is pinned to one class rather than to a set of permitted ones.
    pub cases_with_pinned_class: usize,
    /// Every verdict the run observed, counted by class, plus `accepted`.
    pub outcomes: Vec<(&'static str, usize)>,
    /// Cases dropped before execution because their canonical bytes matched the original's.
    pub degenerate_dropped: usize,
}

impl Coverage {
    pub fn is_exhaustive(&self) -> bool {
        self.position_step == 1
    }

    /// The sentence a test prints so the bound is stated wherever the number is.
    pub fn bound_statement(&self) -> String {
        if self.is_exhaustive() {
            format!(
                "{}: exhaustive over all {} positions, {} digest fields, {} digest offsets, {} cases ({} pinned to one refusal class) (seed {})",
                self.label,
                self.positions_total,
                self.digest_fields,
                self.digest_offsets_covered,
                self.cases,
                self.cases_with_pinned_class,
                self.seed
            )
        } else {
            format!(
                "{}: BOUNDED to every {}th JSON pointer in document order, {} of {} positions; digest coverage stays exhaustive at {} fields and {} offsets; {} cases ({} pinned to one refusal class) (seed {})",
                self.label,
                self.position_step,
                self.positions_covered,
                self.positions_total,
                self.digest_fields,
                self.digest_offsets_covered,
                self.cases,
                self.cases_with_pinned_class,
                self.seed
            )
        }
    }
}

impl fmt::Display for Coverage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.bound_statement())
    }
}

/// A case whose verdict did not match the claim its generator made about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hole {
    pub label: &'static str,
    pub seed: u64,
    pub mutator: &'static str,
    pub pointer: String,
    pub description: String,
    pub expect: Expect,
    pub observed: Verdict,
}

impl Hole {
    /// Whether the verifier refused this case but named a class the expectation forbids.
    ///
    /// A hole of this shape is not a document that got through. It is a document that was stopped
    /// and then described to its reader as something it is not, which is the failure mode the
    /// class distinction exists to prevent.
    pub fn is_wrong_reason(&self) -> bool {
        !self.observed.is_accepted() && matches!(self.expect, Expect::Rejected(_))
    }
}

impl fmt::Display for Hole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{} seed={} mutator={} pointer={}] {} — expected {}, observed {}",
            self.label,
            self.seed,
            self.mutator,
            if self.pointer.is_empty() {
                "<root>"
            } else {
                &self.pointer
            },
            self.description,
            self.expect,
            self.observed
        )
    }
}

/// What one battery run measured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatteryOutcome {
    pub coverage: Coverage,
    /// The verdict on the unmutated document. Anything but [`Verdict::Accepted`] makes the rest
    /// of the run meaningless, and is reported as its own hole rather than silently tolerated.
    pub baseline: Verdict,
    pub holes: Vec<Hole>,
    /// Formatting-only cases whose canonical bytes moved. A non-empty list is a defect in
    /// canonicalisation itself, not in the verifier under test.
    pub canonicalisation_violations: Vec<Hole>,
}

impl BatteryOutcome {
    pub fn is_clean(&self) -> bool {
        self.baseline.is_accepted()
            && self.holes.is_empty()
            && self.canonicalisation_violations.is_empty()
    }

    /// The holes where the verifier refused the case and named a class the expectation forbids.
    pub fn wrong_reason_holes(&self) -> Vec<&Hole> {
        self.holes
            .iter()
            .filter(|hole| hole.is_wrong_reason())
            .collect()
    }

    /// The multi-line message a failing test prints: the bound, the verdicts the run actually saw,
    /// then every hole with its seed.
    ///
    /// The verdict counts are there because a hole is usually one instance of a pattern, and the
    /// pattern is easier to see in the distribution than in the first failing case: a verifier that
    /// has started answering `digest_malformed` to body edits shows up as a class that should have
    /// been at zero.
    pub fn report(&self) -> String {
        let counts: Vec<String> = self
            .coverage
            .outcomes
            .iter()
            .map(|(outcome, count)| format!("{outcome} {count}"))
            .collect();
        let mut lines = vec![
            self.coverage.bound_statement(),
            format!("verdicts: {}", counts.join(", ")),
        ];
        if !self.baseline.is_accepted() {
            lines.push(format!(
                "baseline: the unmutated document was {} — the run measured nothing",
                self.baseline
            ));
        }
        for violation in &self.canonicalisation_violations {
            lines.push(format!("canonicalisation: {violation}"));
        }
        for hole in &self.holes {
            lines.push(format!("hole: {hole}"));
        }
        lines.join("\n")
    }
}

/// How wide a battery run is allowed to be, and how sharply it may judge a refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatteryConfig {
    /// Names the document type in every failure message.
    pub label: &'static str,
    pub seed: u64,
    /// Maximum positions the structural families may visit. `0` means no bound. Digest coverage
    /// is never bounded by this.
    pub position_cap: usize,
    /// The pointer to the field whose value seals the document, where it has one. Knowing it is
    /// what lets the battery demand a specific refusal instead of any refusal.
    pub sealing_digest: Option<&'static str>,
    /// The classes this verifier is allowed to answer with for an edit that moved the body and
    /// left the claimed digest untouched.
    pub body_edit_classes: &'static [RejectionClass],
}

impl BatteryConfig {
    pub fn exhaustive(label: &'static str, seed: u64) -> Self {
        BatteryConfig {
            label,
            seed,
            position_cap: 0,
            sealing_digest: None,
            body_edit_classes: ANY_CLASS,
        }
    }

    pub fn bounded(label: &'static str, seed: u64, position_cap: usize) -> Self {
        BatteryConfig {
            label,
            seed,
            position_cap,
            sealing_digest: None,
            body_edit_classes: ANY_CLASS,
        }
    }

    /// Names the field whose value seals the document, and narrows body edits to the classes a
    /// verifier that keeps the digest distinctions is allowed to answer with.
    pub fn sealed_by(mut self, pointer: &'static str) -> Self {
        self.sealing_digest = Some(pointer);
        self.body_edit_classes = BODY_EDIT_CLASSES;
        self
    }

    /// Overrides the classes permitted for an edit that moved the body.
    pub fn body_edits_reported_as(mut self, classes: &'static [RejectionClass]) -> Self {
        self.body_edit_classes = classes;
        self
    }
}

/// The value standing at `sealing` once `patch` is applied, without applying it.
///
/// A patch either contains the sealing digest's position or is disjoint from it, so the answer is
/// a lookup in one of two places and never a copy of the document.
fn sealing_after<'a>(
    document: &'a Value,
    patch: &'a walk::Patch,
    sealing: &str,
) -> Option<&'a Value> {
    if sealing == patch.target {
        return Some(&patch.value);
    }
    if let Some(rest) = sealing.strip_prefix(patch.target.as_str()) {
        if patch.target.is_empty() || rest.starts_with('/') {
            return patch.value.pointer(rest);
        }
    }
    walk::get(document, sealing)
}

/// Whether `case` leaves the value at `sealing` exactly as the document carries it.
///
/// This is the question that separates a statement about the claimed digest from a statement about
/// the body, and it is not the same as asking whether the case's pointer is the digest's: deleting
/// a root key patches the root, so a patch that carries the digest along untouched looks, from the
/// pointer alone, like it might have moved it.
pub fn leaves_sealing_digest_untouched(document: &Value, case: &Mutation, sealing: &str) -> bool {
    sealing_after(document, &case.patch, sealing) == walk::get(document, sealing)
}

/// The specific answer `case` has to produce, given what it did to the document's sealing digest.
///
/// A generator says only *refused*, because it does not know which field seals the document it was
/// handed. This adds that knowledge. The distinction it turns on is whether the edit touched the
/// sealing digest and nothing else: an edit that did is a statement about the claimed digest alone
/// and has exactly one correct answer, while an edit that moved the body as well leaves a verifier
/// free to refuse the document for the earlier reason it found — as long as that reason does not
/// blame a digest that is intact.
pub fn refine(config: &BatteryConfig, document: &Value, case: &Mutation) -> Expect {
    let Expect::Rejected(Refusal::Any) = case.expect else {
        return case.expect;
    };
    let permitted = Refusal::OneOf(config.body_edit_classes);
    let Some(sealing) = config.sealing_digest else {
        return Expect::Rejected(permitted);
    };
    let only_the_digest_moved = case.pointer == sealing;
    let refusal = match sealing_after(document, &case.patch, sealing) {
        None if only_the_digest_moved => Refusal::Exactly(RejectionClass::DigestAbsent),
        None => Refusal::OneOf(ABSENT_OR_UNREADABLE),
        Some(Value::String(claimed)) if claimed.is_empty() => Refusal::OneOf(ABSENT_OR_MALFORMED),
        Some(Value::String(claimed)) if ContentHash::parse(claimed.clone()).is_err() => {
            if only_the_digest_moved {
                Refusal::Exactly(RejectionClass::DigestMalformed)
            } else {
                Refusal::OneOf(MALFORMED_OR_UNREADABLE)
            }
        }
        Some(Value::String(claimed)) => {
            let issued = walk::get(document, sealing).and_then(Value::as_str);
            if issued == Some(claimed.as_str()) {
                permitted
            } else if only_the_digest_moved {
                Refusal::Exactly(RejectionClass::DigestMismatch)
            } else {
                permitted
            }
        }
        Some(_) => Refusal::OneOf(ABSENT_OR_MALFORMED),
    };
    Expect::Rejected(refusal)
}

/// What running a batch of cases against one verifier produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseRun {
    /// The verdict on the unmutated document.
    pub baseline: Verdict,
    pub holes: Vec<Hole>,
    pub canonicalisation_violations: Vec<Hole>,
    /// Cases removed before execution because their canonical bytes matched the original's.
    pub degenerate_dropped: usize,
    pub cases_with_pinned_class: usize,
    pub outcomes: Vec<(&'static str, usize)>,
}

/// Run every case in `cases` against `verify`, refining each expectation first.
///
/// `cases` is left holding exactly the cases that ran: a mutation whose canonical bytes match the
/// original's is removed rather than asserted on, because a digest cannot distinguish a document
/// from itself and claiming such a case was refused would be claiming something untrue.
///
/// One document is kept and each case is swapped into it and back out again, so a run of thousands
/// of cases costs no copies of the document at all. That is what makes an exhaustive sweep of a
/// large receipt affordable; the alternative — a mutated document per case — spends the whole
/// budget on `clone`.
pub fn run_cases(
    document: &Value,
    config: &BatteryConfig,
    cases: &mut Vec<Mutation>,
    verify: &dyn Fn(&Value) -> Verdict,
) -> CaseRun {
    let baseline = verify(document);
    let degenerate_dropped = mutators::drop_degenerate(document, cases);

    let mut holes = Vec::new();
    let mut canonicalisation_violations = Vec::new();
    let mut cases_with_pinned_class = 0;
    let mut outcomes: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut working = document.clone();

    for case in cases.iter_mut() {
        case.expect = refine(config, document, case);
        if let Expect::Rejected(refusal) = case.expect {
            if refusal.is_pinned() {
                cases_with_pinned_class += 1;
            }
        }
        if !walk::swap_in(&mut working, &mut case.patch) {
            continue;
        }
        let observed = verify(&working);
        walk::swap_in(&mut working, &mut case.patch);

        *outcomes.entry(observed.outcome()).or_default() += 1;
        let hole = |observed: Verdict| Hole {
            label: config.label,
            seed: config.seed,
            mutator: case.mutator,
            pointer: case.pointer.clone(),
            description: case.description.clone(),
            expect: case.expect,
            observed,
        };
        match case.expect {
            Expect::VerdictUnchanged => {
                if mutators::moves_canonical_bytes(document, case) {
                    canonicalisation_violations.push(hole(observed.clone()));
                }
                if observed != baseline {
                    holes.push(hole(observed));
                }
            }
            Expect::Rejected(refusal) => match observed.class() {
                Some(class) if refusal.permits(class) => {}
                _ => holes.push(hole(observed)),
            },
        }
    }

    CaseRun {
        baseline,
        holes,
        canonicalisation_violations,
        degenerate_dropped,
        cases_with_pinned_class,
        outcomes: outcomes.into_iter().collect(),
    }
}

fn tally(cases: &[Mutation]) -> Vec<(&'static str, usize)> {
    mutators::MUTATORS
        .iter()
        .map(|mutator| {
            (
                *mutator,
                cases.iter().filter(|case| case.mutator == *mutator).count(),
            )
        })
        .collect()
}

/// Generate the battery for `document` and run every case through `verify`.
///
/// The document is expected to be one the verifier accepts; a battery over a document that was
/// already invalid would report holes that are really artefacts of the starting point, so the
/// baseline verdict is taken first and carried in the outcome.
pub fn run_battery(
    document: &Value,
    config: &BatteryConfig,
    verify: &dyn Fn(&Value) -> Verdict,
) -> BatteryOutcome {
    let all_positions = walk::pointers(document);
    let digests = mutators::digest_pointers_among(document, &all_positions);
    let (positions, position_step) = walk::strided(&all_positions, config.position_cap);

    let mut rng = SplitMix64::new(config.seed);
    let mut cases = mutators::generate(document, &positions, &digests, &mut rng);
    let run = run_cases(document, config, &mut cases, verify);

    BatteryOutcome {
        coverage: Coverage {
            label: config.label,
            seed: config.seed,
            positions_total: all_positions.len(),
            positions_covered: positions.len(),
            position_step,
            digest_fields: digests.len(),
            digest_offsets_covered: cases
                .iter()
                .filter(|case| case.mutator == "digest_byte_flip")
                .count(),
            cases: cases.len(),
            cases_by_mutator: tally(&cases),
            cases_with_pinned_class: run.cases_with_pinned_class,
            outcomes: run.outcomes,
            degenerate_dropped: run.degenerate_dropped,
        },
        baseline: run.baseline,
        holes: run.holes,
        canonicalisation_violations: run.canonicalisation_violations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::BTreeSet;

    fn sealed() -> Value {
        let mut document = json!({
            "schema": "audit/self-test/0.1",
            "rows": [{ "id": "one", "weight": 1 }, { "id": "two", "weight": 9007199254740992u64 }],
            "note": "a sealed document",
        });
        let digest = ContentHash::of_value(&document)
            .expect("canonicalises")
            .to_string();
        document["sha256"] = Value::String(digest);
        document
    }

    fn config() -> BatteryConfig {
        BatteryConfig::exhaustive("self-test", 0xA11CE).sealed_by("/sha256")
    }

    /// A verifier written the way the workspace writes them: strip the digest, recompute, compare,
    /// and keep a shape defect in the claimed digest distinct from a mismatch.
    fn honest(document: &Value) -> Verdict {
        let Some(object) = document.as_object() else {
            return Verdict::rejected(RejectionClass::Malformed, "not an object");
        };
        let Some(claimed) = object.get("sha256").and_then(Value::as_str) else {
            return Verdict::rejected(RejectionClass::DigestAbsent, "no sha256");
        };
        if ContentHash::parse(claimed.to_string()).is_err() {
            return Verdict::rejected(RejectionClass::DigestMalformed, "sha256 is not 64 hex");
        }
        let mut body = object.clone();
        body.remove("sha256");
        match ContentHash::of_value(&Value::Object(body)) {
            Ok(recomputed) if recomputed.as_str() == claimed => Verdict::Accepted,
            Ok(_) => Verdict::rejected(RejectionClass::DigestMismatch, "recomputation differs"),
            Err(error) => Verdict::rejected(RejectionClass::Malformed, error.to_string()),
        }
    }

    /// The projection a shallow verifier mistakes for the document.
    ///
    /// Two keys copied across and every number folded into one `f64` total — which is where its
    /// second flaw lives, because an `f64` cannot tell two integers apart past the fifty-third bit.
    fn covered_projection(document: &Value) -> Value {
        let Some(object) = document.as_object() else {
            return Value::Null;
        };
        let total: f64 = object
            .get("rows")
            .and_then(Value::as_array)
            .map(|rows| {
                rows.iter()
                    .filter_map(|row| row.get("weight").and_then(Value::as_f64))
                    .sum()
            })
            .unwrap_or_default();
        json!({
            "schema": object.get("schema"),
            "note": object.get("note"),
            "weight_total": total,
        })
    }

    /// The same verifier with the flaws the battery exists to catch.
    ///
    /// It really does compare something, and refuses a document whose comparison moves — it is not
    /// a verifier that says yes to everything, which would be too easy a target to prove anything
    /// with. What it compares is a projection of two keys and one floating-point total, so an edit
    /// the projection does not reach gets through, and so does an edit to a number that an `f64`
    /// rounds back onto the value it replaced. Both are holes the battery has to report: a
    /// self-test that cannot fail is worth nothing, so this verifier is kept exactly as weak as the
    /// assertions are strong.
    fn shallow(document: &Value) -> Verdict {
        let Some(object) = document.as_object() else {
            return Verdict::rejected(RejectionClass::Malformed, "not an object");
        };
        let Some(claimed) = object.get("sha256").and_then(Value::as_str) else {
            return Verdict::rejected(RejectionClass::DigestAbsent, "no sha256");
        };
        if claimed.len() != 64 {
            return Verdict::rejected(RejectionClass::DigestMalformed, "sha256 is not 64 long");
        }
        let (Ok(here), Ok(issued)) = (
            ContentHash::of_value(&covered_projection(document)),
            ContentHash::of_value(&covered_projection(&sealed())),
        ) else {
            return Verdict::rejected(RejectionClass::Malformed, "unhashable");
        };
        if here == issued {
            Verdict::Accepted
        } else {
            Verdict::rejected(RejectionClass::DigestMismatch, "the projection moved")
        }
    }

    /// A verifier that catches every edit and describes every one of them the same way.
    ///
    /// Nothing gets past it, so a battery that only asked *was this refused?* would call it
    /// perfect. It is not: it tells the holder of a receipt whose digest was never a digest, and
    /// the holder of one that has no digest at all, that the body was tampered with.
    fn blames_the_body(document: &Value) -> Verdict {
        match honest(document) {
            Verdict::Accepted => Verdict::Accepted,
            Verdict::Rejected { detail, .. } => {
                Verdict::rejected(RejectionClass::DigestMismatch, detail)
            }
        }
    }

    #[test]
    fn an_honest_verifier_survives_the_whole_battery() {
        let outcome = run_battery(&sealed(), &config(), &honest);
        assert!(outcome.is_clean(), "{}", outcome.report());
        assert_eq!(outcome.baseline, Verdict::Accepted);
        assert!(outcome.coverage.cases > 100, "{}", outcome.coverage);
        assert!(
            outcome.coverage.cases_with_pinned_class >= mutators::DIGEST_CHARS,
            "every offset of the sealing digest pins one class: {}",
            outcome.coverage
        );
    }

    /// Every semantic family, not a sample of three.
    ///
    /// The shallow verifier is the one deliberately broken reader this crate ships, and it is the
    /// only evidence that a family still detects anything. Naming three of them left the other
    /// eight free to stop catching it — to start generating cases the verifier happens to refuse,
    /// or none at all — without a single test going red. So the demand is on all of them, and the
    /// list is derived from what the families themselves claim rather than retyped, so a family
    /// added later arrives already under it.
    #[test]
    fn a_verifier_that_hashes_only_the_keys_it_knows_is_caught_by_every_semantic_family() {
        let outcome = run_battery(&sealed(), &config(), &shallow);
        assert!(!outcome.is_clean());
        assert!(
            outcome.holes.len() > 20,
            "a shallow verifier must fail widely, not once: {}",
            outcome.report()
        );
        let document = sealed();
        let positions = walk::pointers(&document);
        let digests = mutators::digest_pointers(&document);
        let mut rng = SplitMix64::new(config().seed);
        let generated = mutators::generate(&document, &positions, &digests, &mut rng);

        let claims_a_refusal: BTreeSet<&str> = generated
            .iter()
            .filter(|case| matches!(case.expect, Expect::Rejected(_)))
            .map(|case| case.mutator)
            .collect();
        let claims_invariance: Vec<&str> = mutators::MUTATORS
            .iter()
            .copied()
            .filter(|mutator| !claims_a_refusal.contains(mutator))
            .collect();
        assert_eq!(
            claims_invariance,
            vec!["object_key_reordering"],
            "the demand below is on every family that claims a refusal, and the one exemption is \
             derived rather than named: canonical JSON sorts object keys, so reordering them \
             claims the verdict is unchanged and a hole would mean the verifier moved, not that \
             the family detected anything. Any other family reaching this list has quietly \
             stopped claiming a refusal"
        );
        let semantic: Vec<&str> = claims_a_refusal
            .iter()
            .copied()
            .filter(|mutator| !mutator.starts_with("digest_"))
            .collect();
        assert_eq!(
            semantic.len(),
            claims_a_refusal.len() - 3,
            "the three digest families are held out because the shallow verifier is not shallow \
             about the seal: it checks the claimed digest's length and compares a recomputation, \
             so it catches them, and a family producing no hole there is the family working"
        );
        for mutator in semantic {
            assert!(
                outcome.holes.iter().any(|hole| hole.mutator == mutator),
                "{mutator} did not catch the shallow verifier at any position. Either the family \
                 stopped generating cases or it stopped generating ones a key-blind reader \
                 misses, and neither is visible from a test that names only some families: {}",
                outcome.report()
            );
        }
    }

    /// The shallow verifier reads its numbers through an `f64`, so the two integers either side of
    /// the exact-integer boundary are one number to it. The boundary family exists to say so.
    #[test]
    fn a_verifier_that_reads_a_number_through_a_float_is_caught_past_the_exact_integer_boundary() {
        let outcome = run_battery(&sealed(), &config(), &shallow);
        let caught: Vec<&Hole> = outcome
            .holes
            .iter()
            .filter(|hole| {
                hole.mutator == "numeric_boundary"
                    && hole
                        .description
                        .contains(&mutators::FIRST_INEXACT_INTEGER.to_string())
            })
            .collect();
        assert!(
            !caught.is_empty(),
            "the first integer no f64 holds exactly slipped past: {}",
            outcome.report()
        );
        assert!(caught.iter().all(|hole| hole.observed.is_accepted()));
    }

    #[test]
    fn a_verifier_that_answers_every_refusal_with_tampering_is_caught_by_the_class_it_names() {
        let outcome = run_battery(&sealed(), &config(), &blames_the_body);
        assert!(
            !outcome.is_clean(),
            "a verifier that calls an absent digest tampering must not pass"
        );
        assert!(
            outcome.holes.iter().all(|hole| hole.is_wrong_reason()),
            "nothing got through it; every hole is a refusal that named the wrong class: {}",
            outcome.report()
        );
        let absent = outcome
            .holes
            .iter()
            .find(|hole| hole.pointer == "/sha256" && hole.mutator == "required_key_deletion")
            .expect("deleting the sealing digest is one of the generated cases");
        assert_eq!(
            absent.expect,
            Expect::Rejected(Refusal::Exactly(RejectionClass::DigestAbsent))
        );
        assert_eq!(
            absent.observed.class(),
            Some(RejectionClass::DigestMismatch)
        );
    }

    #[test]
    fn a_battery_told_nothing_about_the_sealing_digest_demands_only_that_a_case_be_refused() {
        let unrefined = run_battery(
            &sealed(),
            &BatteryConfig::exhaustive("self-test", 0xA11CE),
            &blames_the_body,
        );
        assert!(
            unrefined.is_clean(),
            "without a sealing digest there is no class to demand: {}",
            unrefined.report()
        );
        assert_eq!(unrefined.coverage.cases_with_pinned_class, 0);
    }

    #[test]
    fn refinement_pins_the_class_a_digest_edit_has_to_produce() {
        let document = sealed();
        let config = config();
        let mut rng = SplitMix64::new(1);
        let digests = mutators::digest_pointers(&document);
        let expectations = |cases: Vec<Mutation>| -> Vec<Expect> {
            cases
                .iter()
                .filter(|case| case.pointer == "/sha256")
                .map(|case| refine(&config, &document, case))
                .collect()
        };

        let flips = expectations(mutators::digest_byte_flips(&document, &digests, &mut rng));
        assert_eq!(flips.len(), mutators::DIGEST_CHARS);
        assert!(flips
            .iter()
            .all(|expect| *expect
                == Expect::Rejected(Refusal::Exactly(RejectionClass::DigestMismatch))));

        let recased = expectations(mutators::digest_case_changes(&document, &digests));
        assert!(!recased.is_empty());
        assert!(recased
            .iter()
            .all(|expect| *expect
                == Expect::Rejected(Refusal::Exactly(RejectionClass::DigestMalformed))));

        let deleted = expectations(mutators::required_key_deletions(
            &document,
            &walk::pointers(&document),
        ));
        assert_eq!(
            deleted,
            vec![Expect::Rejected(Refusal::Exactly(
                RejectionClass::DigestAbsent
            ))]
        );
    }

    #[test]
    fn refinement_never_lets_a_body_edit_be_blamed_on_the_digest_that_sealed_it() {
        let document = sealed();
        let config = config();
        let cases = mutators::empty_or_null_substitutions(&document, &walk::pointers(&document));
        let body_edits: Vec<&Mutation> = cases
            .iter()
            .filter(|case| !case.pointer.starts_with("/sha256"))
            .collect();
        assert!(!body_edits.is_empty());
        for case in body_edits {
            let Expect::Rejected(refusal) = refine(&config, &document, case) else {
                panic!("a substitution is a semantic edit");
            };
            assert!(
                !refusal.permits(RejectionClass::DigestAbsent),
                "{}",
                case.description
            );
            assert!(
                !refusal.permits(RejectionClass::DigestMalformed),
                "{}",
                case.description
            );
            assert!(refusal.permits(RejectionClass::DigestMismatch));
        }
    }

    #[test]
    fn digest_coverage_stays_exhaustive_when_the_position_budget_is_bound() {
        let document = sealed();
        let bounded = run_battery(
            &document,
            &BatteryConfig::bounded("self-test", 7, 3).sealed_by("/sha256"),
            &honest,
        );
        assert!(!bounded.coverage.is_exhaustive());
        assert!(bounded.coverage.positions_covered < bounded.coverage.positions_total);
        assert!(bounded.coverage.bound_statement().contains("BOUNDED"));

        let exhaustive = run_battery(
            &document,
            &BatteryConfig::exhaustive("self-test", 7).sealed_by("/sha256"),
            &honest,
        );
        assert_eq!(
            bounded.coverage.digest_offsets_covered,
            exhaustive.coverage.digest_offsets_covered
        );
        assert_eq!(
            bounded.coverage.digest_offsets_covered,
            mutators::DIGEST_CHARS
        );
    }

    #[test]
    fn the_same_seed_reruns_the_same_cases_and_reports_the_same_coverage() {
        let document = sealed();
        let config = config();
        let first = run_battery(&document, &config, &honest);
        let second = run_battery(&document, &config, &honest);
        assert_eq!(first, second);
    }

    /// The runner swaps each patch into one shared document and swaps it back out. If a swap were
    /// ever left unpaired, the next case would be measured against a document carrying the
    /// previous case's edit, and the battery would be reporting on documents nobody generated.
    #[test]
    fn every_case_is_verified_against_the_document_it_describes_and_nothing_it_inherited() {
        let document = sealed();
        let seen = std::cell::RefCell::new(Vec::new());
        let outcome = run_battery(&document, &config(), &|candidate| {
            seen.borrow_mut().push(candidate.clone());
            honest(candidate)
        });
        assert!(outcome.is_clean(), "{}", outcome.report());

        let seen = seen.into_inner();
        assert_eq!(seen.first(), Some(&document), "the baseline runs unmutated");
        let mut cases = mutators::generate(
            &document,
            &walk::pointers(&document),
            &mutators::digest_pointers(&document),
            &mut SplitMix64::new(0xA11CE),
        );
        mutators::drop_degenerate(&document, &mut cases);
        assert_eq!(seen.len(), cases.len() + 1);
        for (case, candidate) in cases.iter().zip(seen.iter().skip(1)) {
            assert_eq!(
                *candidate,
                case.applied(&document),
                "{} was verified against a different document than it describes",
                case.description
            );
        }
    }

    #[test]
    fn a_baseline_the_verifier_already_refuses_is_reported_rather_than_measured() {
        let mut broken = sealed();
        broken["note"] = json!("edited after sealing");
        let outcome = run_battery(
            &broken,
            &BatteryConfig::exhaustive("self-test", 1).sealed_by("/sha256"),
            &honest,
        );
        assert!(!outcome.is_clean());
        assert_eq!(
            outcome.baseline.class(),
            Some(RejectionClass::DigestMismatch)
        );
        assert!(outcome.report().contains("the run measured nothing"));
    }
}
