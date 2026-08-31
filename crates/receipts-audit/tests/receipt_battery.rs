//! The depth battery: every generator, at every position, against every digest-sealed document
//! type the workspace ships a verifier for.
//!
//! Each test states one property and reports what it measured. The counts in the assertions are
//! pinned rather than bounded from below on purpose: a coverage number that can silently shrink
//! is not a coverage number, and a generator that stops producing cases would otherwise turn this
//! file green by doing nothing.
//!
//! Every rejection is judged by the class it carries, not merely by the fact that it happened. A
//! verifier that stops a tampered receipt and then tells its holder the digest was never a digest
//! has stopped the attack and misdescribed it, and this file treats that as a failure. Which class
//! is correct comes from `BatteryConfig::sealed_by`, which names the field that seals each document
//! so the library can work out what the only right answer is at each position.

mod documents;
mod verifiers;

use bioprism_ids::{to_canonical_string, ContentHash};
use bioprism_receipts_audit::{
    mutators, rng::SplitMix64, run_battery, run_cases, walk, BatteryConfig, CaseRun, Expect,
    GeneratedCase, RejectionClass, Verdict,
};
use bioprism_research::render_report;
use bioprism_section::{CertificateProfile, ContextCertificate};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// One seed for the whole battery. Every failure message repeats it, so a reported hole is a
/// complete reproduction recipe: this seed, that document type, that pointer.
const SEED: u64 = 0x9E37_79B9_7F4A_7C15;

/// The classes `verify_delivery_receipt` may answer with for an edit to the delivery audit a
/// receipt was derived from.
///
/// The audit is not itself sealed — it is the input the receipt is recomputed from — so an edit to
/// it surfaces as whichever comparison notices first: the recomputed receipt digest when the edit
/// reaches the sealed projection, a projection finding when it does not, and a refusal to read the
/// document at all when the edit breaks the shape `build_delivery_receipt` requires.
const DELIVERY_AUDIT_CLASSES: &[RejectionClass] = &[
    RejectionClass::DigestMismatch,
    RejectionClass::StructuralFailure,
    RejectionClass::Malformed,
];

struct Subject {
    label: &'static str,
    document: Value,
    verify: Box<dyn Fn(&Value) -> Verdict>,
    config: BatteryConfig,
}

impl Subject {
    fn positions(&self) -> Vec<String> {
        walk::strided(&walk::pointers(&self.document), self.config.position_cap).0
    }

    fn digests(&self) -> Vec<String> {
        mutators::digest_pointers(&self.document)
    }

    fn sealing_digest(&self) -> Option<&'static str> {
        self.config.sealing_digest
    }

    /// Feeds one family's cases to this subject's verifier under the battery's own rules: the
    /// expectations are refined to the class each case must produce, degenerate cases are dropped,
    /// and one document is reused rather than copied per case.
    fn run(&self, mut cases: Vec<GeneratedCase>) -> FamilyRun {
        let run = run_cases(&self.document, &self.config, &mut cases, &self.verify);
        FamilyRun {
            label: self.label,
            cases,
            run,
        }
    }
}

/// One family's cases after they have been run.
struct FamilyRun {
    label: &'static str,
    cases: Vec<GeneratedCase>,
    run: CaseRun,
}

impl FamilyRun {
    fn executed(&self) -> usize {
        self.cases.len()
    }

    fn excused(&self) -> usize {
        self.run
            .holes
            .iter()
            .filter(|hole| {
                KNOWN_GAPS
                    .iter()
                    .any(|gap| gap.matches(hole.label, hole.mutator, &hole.pointer))
            })
            .count()
    }

    fn unexplained(&self) -> Vec<String> {
        self.run
            .holes
            .iter()
            .filter(|hole| {
                !KNOWN_GAPS
                    .iter()
                    .any(|gap| gap.matches(hole.label, hole.mutator, &hole.pointer))
            })
            .map(ToString::to_string)
            .collect()
    }

    /// Panics with every hole this family produced that no recorded gap explains.
    fn assert_no_unexplained_hole(&self) {
        let unexplained = self.unexplained();
        assert!(
            unexplained.is_empty(),
            "{} (seed {SEED}): {} of {} cases answered outside their expectation\n{}",
            self.label,
            unexplained.len(),
            self.executed(),
            unexplained.join("\n")
        );
    }
}

fn subjects() -> Vec<Subject> {
    let delivery = documents::delivery_audit();
    let receipt = documents::delivery_receipt();
    let receipt_for_audit = receipt.clone();
    vec![
        Subject {
            label: "context_certificate",
            document: documents::certificate(),
            verify: Box::new(verifiers::certificate),
            config: BatteryConfig::exhaustive("context_certificate", SEED)
                .sealed_by("/certificate_sha256"),
        },
        Subject {
            label: "autopilot_report",
            document: documents::autopilot_report(),
            verify: Box::new(verifiers::autopilot),
            config: BatteryConfig::exhaustive("autopilot_report", SEED).sealed_by("/report_sha256"),
        },
        Subject {
            label: "research_dossier",
            document: documents::dossier().clone(),
            verify: Box::new(verifiers::dossier),
            config: BatteryConfig::exhaustive("research_dossier", SEED)
                .sealed_by("/dossier_sha256"),
        },
        Subject {
            label: "mission_evidence_bundle",
            document: documents::evidence_bundle(),
            verify: Box::new(verifiers::evidence_bundle),
            config: BatteryConfig::exhaustive("mission_evidence_bundle", SEED)
                .sealed_by("/bundle_digest"),
        },
        Subject {
            label: "delivery_receipt",
            document: receipt,
            verify: Box::new(move |document| verifiers::delivery_receipt(document, &delivery)),
            config: BatteryConfig::exhaustive("delivery_receipt", SEED)
                .sealed_by("/receipt_digest"),
        },
        Subject {
            label: "delivery_audit_behind_a_fixed_receipt",
            document: documents::delivery_audit(),
            verify: Box::new(move |document| {
                verifiers::delivery_receipt(&receipt_for_audit, document)
            }),
            config: BatteryConfig::exhaustive("delivery_audit_behind_a_fixed_receipt", SEED)
                .body_edits_reported_as(DELIVERY_AUDIT_CLASSES),
        },
    ]
}

/// A case the battery generates, a verifier answers wrongly, and this repository has decided not
/// to close — recorded here so the exemption is visible, justified, and load-bearing.
///
/// A gap on this list is asserted twice over: cases matching it are excused from the hole count,
/// *and* the gap must still fire. Closing the underlying behaviour without deleting the entry
/// fails the battery, so the list cannot rot into a set of stale excuses.
struct KnownGap {
    label: &'static str,
    mutator: &'static str,
    pointer: &'static str,
    reason: &'static str,
}

const KNOWN_GAPS: [KnownGap; 1] = [KnownGap {
    label: "delivery_receipt",
    mutator: "unexpected_key",
    pointer: "",
    reason: "verify_delivery_receipt compares every field the recomputation produces and ignores \
             fields it does not: the shipped MCP surface returns the receipt with ok, workflow, \
             valid, receipt_ready, and delivery written onto the same object, so treating an \
             unrecognised key as tampering would reject every receipt the server hands out. An \
             unrecognised key at the root of a receipt is therefore not checked at all",
}];

impl KnownGap {
    fn matches(&self, label: &str, mutator: &str, pointer: &str) -> bool {
        self.label == label && self.mutator == mutator && self.pointer == pointer
    }
}

fn wrong_digest(document: &Value, pointer: &str) -> Value {
    let claimed = walk::get(document, pointer)
        .and_then(Value::as_str)
        .expect("the sealing digest is a string");
    let replacement: String = claimed
        .chars()
        .map(|c| if c == 'a' { 'b' } else { 'a' })
        .collect();
    assert_ne!(replacement, claimed);
    walk::with_replacement(document, pointer, Value::String(replacement))
        .expect("the sealing digest is replaceable")
}

// -- the headline property --------------------------------------------------------------------

#[test]
fn the_whole_battery_finds_no_hole_outside_the_gaps_this_repository_has_named() {
    let mut total_cases = 0;
    let mut total_positions = 0;
    let mut total_pinned = 0;
    let mut bounds = Vec::new();
    let mut gaps_fired = vec![0usize; KNOWN_GAPS.len()];
    for subject in subjects() {
        let outcome = run_battery(&subject.document, &subject.config, &subject.verify);
        assert!(
            outcome.canonicalisation_violations.is_empty(),
            "{}",
            outcome.report()
        );
        assert_eq!(
            outcome.baseline,
            Verdict::Accepted,
            "{}: {}",
            subject.label,
            outcome.baseline
        );
        assert!(
            outcome.coverage.is_exhaustive(),
            "{}: the sweep visits every position of every subject: {}",
            subject.label,
            outcome.coverage
        );
        let mut unexplained = Vec::new();
        for hole in &outcome.holes {
            match KNOWN_GAPS
                .iter()
                .position(|gap| gap.matches(hole.label, hole.mutator, &hole.pointer))
            {
                Some(index) => gaps_fired[index] += 1,
                None => unexplained.push(hole.to_string()),
            }
        }
        assert!(
            unexplained.is_empty(),
            "{}\n{}",
            outcome.coverage.bound_statement(),
            unexplained.join("\n")
        );
        assert!(
            outcome.coverage.cases > 100,
            "{} generated only {} cases",
            subject.label,
            outcome.coverage.cases
        );
        total_cases += outcome.coverage.cases;
        total_positions += outcome.coverage.positions_covered;
        total_pinned += outcome.coverage.cases_with_pinned_class;
        bounds.push(outcome.coverage.bound_statement());
    }
    for (gap, fired) in KNOWN_GAPS.iter().zip(&gaps_fired) {
        assert!(
            *fired > 0,
            "the recorded gap on {} / {} no longer fires — delete the entry rather than leaving a \
             stale excuse in the battery. Its reason was: {}",
            gap.label,
            gap.mutator,
            gap.reason
        );
    }
    assert_eq!(
        (total_cases, total_positions, total_pinned),
        (18_320, 2_275, 375),
        "the battery's coverage is a pinned claim; bounds were:\n{}",
        bounds.join("\n")
    );
}

/// No subject is bounded, so nothing in this file needs the sentence that used to say so.
///
/// The dossier was the one document the sweep did not cover: 1,981 positions, visited every eighth
/// pointer. The bound is gone, and this asserts it stays gone — a cap reintroduced for speed would
/// otherwise be invisible in a file whose other numbers all move together.
#[test]
fn no_subject_carries_a_position_bound() {
    for subject in subjects() {
        assert_eq!(
            subject.config.position_cap, 0,
            "{} is bounded to {} positions",
            subject.label, subject.config.position_cap
        );
        assert_eq!(
            subject.positions().len(),
            walk::pointers(&subject.document).len(),
            "{} visits fewer positions than it has",
            subject.label
        );
    }
}

// -- digest integrity, exhaustively -----------------------------------------------------------

#[test]
fn every_single_byte_digest_mutation_is_caught_at_every_offset_of_every_digest_field() {
    let mut fields = 0;
    let mut offsets = 0;
    let mut sealing_offsets = 0;
    for subject in subjects() {
        let found = subject.digests();
        let mut rng = SplitMix64::new(SEED);
        let family = subject.run(mutators::digest_byte_flips(
            &subject.document,
            &found,
            &mut rng,
        ));
        assert_eq!(
            family.executed(),
            found.len() * mutators::DIGEST_CHARS,
            "{}: digest coverage must be exhaustive over offsets, never sampled",
            subject.label
        );
        family.assert_no_unexplained_hole();
        if let Some(sealing) = subject.sealing_digest() {
            let pinned: Vec<&GeneratedCase> = family
                .cases
                .iter()
                .filter(|case| case.pointer == sealing)
                .collect();
            assert_eq!(
                pinned.len(),
                mutators::DIGEST_CHARS,
                "{}: the sealing digest is checked at every offset",
                subject.label
            );
            for case in pinned {
                assert_eq!(
                    case.expect,
                    Expect::Rejected(bioprism_receipts_audit::Refusal::Exactly(
                        RejectionClass::DigestMismatch
                    )),
                    "{}: a well-formed digest over an untouched body has one correct answer",
                    subject.label
                );
            }
            sealing_offsets += mutators::DIGEST_CHARS;
        }
        fields += found.len();
        offsets += family.executed();
    }
    assert_eq!(
        (fields, offsets, sealing_offsets),
        (51, 3_264, 320),
        "51 digest fields across six documents, each checked at all 64 offsets; the 320 offsets of \
         the five sealing digests must each be reported as a mismatch and nothing else"
    );
}

#[test]
fn a_truncated_extended_or_recased_digest_is_caught_at_every_digest_field() {
    let mut cases_run = 0;
    for subject in subjects() {
        let digests = subject.digests();
        let mut cases = mutators::digest_length_changes(&subject.document, &digests);
        cases.extend(mutators::digest_case_changes(&subject.document, &digests));
        let family = subject.run(cases);
        family.assert_no_unexplained_hole();
        cases_run += family.executed();
    }
    assert_eq!(
        cases_run, 357,
        "51 digest fields, seven shape mutations each"
    );
}

/// A digest whose shape is broken is a defect in the claimed digest; a digest whose value is wrong
/// is evidence that the body moved. Every one of these cases breaks the shape of the field that
/// seals the document, so every one of them has exactly one correct answer, and it is never the one
/// that accuses the holder of the receipt.
#[test]
fn a_shape_break_in_the_sealing_digest_is_reported_as_malformed_and_never_as_tampering() {
    let mut checked = 0;
    for subject in subjects() {
        let Some(sealing) = subject.sealing_digest() else {
            continue;
        };
        let mut cases = mutators::digest_length_changes(&subject.document, &[sealing.to_string()]);
        cases.extend(mutators::digest_case_changes(
            &subject.document,
            &[sealing.to_string()],
        ));
        for case in &cases {
            let verdict = (subject.verify)(&case.applied(&subject.document));
            let class = verdict.class();
            let acceptable = if case.description.contains("emptied") {
                // Reading an empty string as no digest at all is defensible and one of the five
                // does; which one is pinned by the table below. The reading none of them may take
                // is that the body was tampered with.
                class == Some(RejectionClass::DigestMalformed)
                    || class == Some(RejectionClass::DigestAbsent)
            } else {
                class == Some(RejectionClass::DigestMalformed)
            };
            assert!(
                acceptable,
                "{} (seed {SEED}): {} was answered with {verdict} — a claimed digest of the wrong \
                 shape was never a digest, and saying the body moved blames the wrong party",
                subject.label, case.description
            );
            checked += 1;
        }
    }
    assert_eq!(
        checked, 35,
        "five sealing digests, seven shape mutations each"
    );
}

// -- canonicalisation invariance --------------------------------------------------------------

#[test]
fn object_key_reordering_never_changes_a_verdict_at_any_position() {
    let mut cases_run = 0;
    for subject in subjects() {
        let baseline = (subject.verify)(&subject.document);
        assert_eq!(
            baseline,
            Verdict::Accepted,
            "{}: the battery starts from a document its verifier accepts",
            subject.label
        );
        let baseline_bytes = to_canonical_string(&subject.document).expect("canonicalises");
        let mut rng = SplitMix64::new(SEED);
        let cases =
            mutators::object_key_reorderings(&subject.document, &subject.positions(), &mut rng);
        assert!(
            !cases.is_empty(),
            "{} produced no reordering",
            subject.label
        );
        for case in &cases {
            assert_eq!(case.expect, Expect::VerdictUnchanged);
            assert_eq!(
                to_canonical_string(&case.applied(&subject.document)).expect("canonicalises"),
                baseline_bytes,
                "{} (seed {SEED}): {} moved the canonical bytes — canonicalisation is not \
                 key-order invariant, and every digest in this workspace is an artefact of one \
                 serializer",
                subject.label,
                case.description
            );
        }
        let family = subject.run(cases);
        assert!(
            family.run.canonicalisation_violations.is_empty(),
            "{}",
            family.unexplained().join("\n")
        );
        family.assert_no_unexplained_hole();
        cases_run += family.executed();
    }
    assert_eq!(cases_run, 472, "reordering cases across six documents");
}

#[test]
fn array_reordering_always_changes_a_verdict_at_any_position() {
    let mut cases_run = 0;
    for subject in subjects() {
        let mut rng = SplitMix64::new(SEED);
        let family = subject.run(mutators::array_reorderings(
            &subject.document,
            &subject.positions(),
            &mut rng,
        ));
        family.assert_no_unexplained_hole();
        cases_run += family.executed();
    }
    assert_eq!(
        cases_run, 163,
        "array reordering cases across six documents — JSON arrays are ordered and a digest that \
         ignores their order is not naming the document"
    );
}

// -- absent, malformed, and mismatching digests stay three different answers -------------------

#[test]
fn a_document_whose_sealing_digest_is_absent_is_rejected_distinctly_from_one_whose_digest_is_wrong()
{
    let mut checked = 0;
    for subject in subjects() {
        let Some(pointer) = subject.sealing_digest() else {
            continue;
        };
        let stripped =
            walk::with_removal(&subject.document, pointer).expect("the digest field is removable");
        let absent = (subject.verify)(&stripped);
        let wrong = (subject.verify)(&wrong_digest(&subject.document, pointer));
        assert_eq!(
            absent.class(),
            Some(RejectionClass::DigestAbsent),
            "{}: a missing {pointer} must be reported as missing, not as tampering — got {absent}",
            subject.label
        );
        assert_eq!(
            wrong.class(),
            Some(RejectionClass::DigestMismatch),
            "{}: a wrong {pointer} must be reported as a mismatch — got {wrong}",
            subject.label
        );
        assert_ne!(absent.class(), wrong.class(), "{}", subject.label);
        checked += 1;
    }
    assert_eq!(checked, 5, "five document types carry a sealing digest");
}

#[test]
fn a_shape_broken_sealing_digest_is_rejected_as_malformed_and_never_as_tampering() {
    let mut checked = 0;
    for subject in subjects() {
        let Some(pointer) = subject.sealing_digest() else {
            continue;
        };
        for broken in [
            "NOT-64-LOWERCASE-HEX-CHARACTERS",
            "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
            "abc",
        ] {
            let document =
                walk::with_replacement(&subject.document, pointer, Value::String(broken.into()))
                    .expect("the digest field is replaceable");
            let verdict = (subject.verify)(&document);
            assert_eq!(
                verdict.class(),
                Some(RejectionClass::DigestMalformed),
                "{}: {pointer} = {broken:?} is a defect in the claimed digest, not evidence that \
                 the body moved — got {verdict}",
                subject.label
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 15);
}

/// Every shape a sealing digest can take that makes it unusable.
///
/// Split into a named list because the table below is the product a receipt holder actually reads:
/// each of these has to come back as *absent* or as *malformed*, and which one is not a detail.
fn unusable_sealing_digests() -> Vec<(&'static str, Option<Value>)> {
    vec![
        ("deleted", None),
        ("the empty string", Some(Value::String(String::new()))),
        ("null", Some(Value::Null)),
        ("a number", Some(serde_json::json!(0))),
        ("an array", Some(serde_json::json!([]))),
        ("an object", Some(serde_json::json!({}))),
        ("a boolean", Some(Value::Bool(true))),
        ("63 hex characters", Some(Value::String("a".repeat(63)))),
        ("65 hex characters", Some(Value::String("a".repeat(65)))),
        (
            "64 uppercase hex characters",
            Some(Value::String("A".repeat(64))),
        ),
        (
            "64 characters that are not hex",
            Some(Value::String("z".repeat(64))),
        ),
        (
            "64 hex characters with surrounding whitespace",
            Some(Value::String(format!(" {} ", "a".repeat(64)))),
        ),
    ]
}

/// The one place the five verifiers disagree about an unusable sealing digest.
///
/// `verify_mission_evidence_bundle` requires `bundle_digest` to be "a non-empty string", so a
/// `bundle_digest` of `""` comes back as *absent*. The other four call it *malformed*, which is the
/// reading that keeps the two answers apart: the field is there, and what is in it was never a
/// digest. This is milder than the holes the battery has closed — it misdescribes a defect rather
/// than blaming the wrong party — and it is recorded here so that fixing it and drifting further
/// both fail loudly.
const EMPTY_DIGEST_READ_AS_ABSENT: &str = "mission_evidence_bundle";

/// The five verifiers answer every unusable sealing digest the same way, with one exception this
/// test names. A digest that is not there is absent; a digest that is there and is not a digest is
/// malformed; a field holding something that is not a string at all is absent, because no digest
/// was supplied. Sixty answers, none of them `digest_mismatch` — nothing here is evidence that a
/// body moved, and a verifier that said so would be accusing the holder of the receipt.
#[test]
fn the_five_verifiers_agree_on_every_unusable_sealing_digest_except_the_empty_string() {
    let mut answers = 0;
    let mut divergences = 0;
    for subject in subjects() {
        let Some(pointer) = subject.sealing_digest() else {
            continue;
        };
        for (label, replacement) in unusable_sealing_digests() {
            let mutated = match replacement {
                None => walk::with_removal(&subject.document, pointer)
                    .expect("the digest field is removable"),
                Some(value) => walk::with_replacement(&subject.document, pointer, value)
                    .expect("the digest field is replaceable"),
            };
            let verdict = (subject.verify)(&mutated);
            let expected = match label {
                "the empty string" if subject.label == EMPTY_DIGEST_READ_AS_ABSENT => {
                    divergences += 1;
                    RejectionClass::DigestAbsent
                }
                "the empty string" => RejectionClass::DigestMalformed,
                "deleted" | "null" | "a number" | "an array" | "an object" | "a boolean" => {
                    RejectionClass::DigestAbsent
                }
                _ => RejectionClass::DigestMalformed,
            };
            assert_eq!(
                verdict.class(),
                Some(expected),
                "{}: {pointer} {label} must be reported as {expected} — got {verdict}",
                subject.label
            );
            answers += 1;
        }
    }
    assert_eq!(
        (answers, divergences),
        (60, 1),
        "twelve unusable digests across five verifiers, one of which reads an empty digest as an \
         absent one"
    );
}

/// The inverse of the malformed rule, and the reason the sweep has to be exhaustive: an edit
/// anywhere in the body leaves the claimed digest exactly as the producer issued it, so no verifier
/// may answer that the digest is absent or defective. It may say the body moved, it may say the
/// document is unreadable, it may say some other contract failed — those name the edit. The two
/// forbidden answers name the digest, and the digest did not move.
///
/// What this pins is the rule itself, at every body position of every sealed document. Holding the
/// verifiers to it is what the family tests above do, because the expectation each of them checks
/// against is the one this test enumerates.
#[test]
fn every_body_edit_forbids_the_two_answers_that_would_blame_the_digest() {
    let mut body_edits = 0;
    for subject in subjects() {
        let Some(sealing) = subject.sealing_digest() else {
            continue;
        };
        let mut rng = SplitMix64::new(SEED);
        let mut cases = mutators::generate(
            &subject.document,
            &subject.positions(),
            &subject.digests(),
            &mut rng,
        );
        mutators::drop_degenerate(&subject.document, &mut cases);
        for case in &cases {
            if case.expect == Expect::VerdictUnchanged
                || !bioprism_receipts_audit::leaves_sealing_digest_untouched(
                    &subject.document,
                    case,
                    sealing,
                )
            {
                continue;
            }
            let Expect::Rejected(refusal) =
                bioprism_receipts_audit::refine(&subject.config, &subject.document, case)
            else {
                panic!("{}: {} is a semantic edit", subject.label, case.description);
            };
            for forbidden in [
                RejectionClass::DigestAbsent,
                RejectionClass::DigestMalformed,
            ] {
                assert!(
                    !refusal.permits(forbidden),
                    "{} (seed {SEED}): {} would be allowed to be reported as {forbidden}, and it \
                     never touched {sealing}",
                    subject.label,
                    case.description
                );
            }
            assert!(refusal.permits(RejectionClass::DigestMismatch));
            body_edits += 1;
        }
    }
    assert_eq!(
        body_edits, 17_310,
        "body edits across the five sealed documents, none of which may be blamed on the digest"
    );
}

#[test]
fn deleting_any_field_at_any_visited_position_is_rejected_and_never_silently_accepted() {
    let mut cases_run = 0;
    for subject in subjects() {
        let mut cases = mutators::required_key_deletions(&subject.document, &subject.positions());
        cases.extend(mutators::array_element_deletions(
            &subject.document,
            &subject.positions(),
        ));
        assert!(!cases.is_empty(), "{} produced no deletion", subject.label);
        let family = subject.run(cases);
        family.assert_no_unexplained_hole();
        cases_run += family.executed();
    }
    assert_eq!(cases_run, 2_269, "deletion cases across six documents");
}

// -- numbers, strings, and structure ----------------------------------------------------------

#[test]
fn a_numeric_near_equal_substitution_lands_on_one_stable_verdict_and_never_between_them() {
    let mut cases_run = 0;
    for subject in subjects() {
        let cases = mutators::numeric_near_equal(&subject.document, &subject.positions());
        for case in &cases {
            let mutated = case.applied(&subject.document);
            let first = (subject.verify)(&mutated);
            let second = (subject.verify)(&mutated);
            assert_eq!(
                first, second,
                "{} (seed {SEED}): {} answered differently on two runs",
                subject.label, case.description
            );
        }
        let family = subject.run(cases);
        family.assert_no_unexplained_hole();
        cases_run += family.executed();
    }
    assert_eq!(
        cases_run, 223,
        "numeric cases across six documents — an integer and its equal-valued float are different \
         canonical bytes, so a verifier that accepted both would make its digest depend on how a \
         caller's parser typed a literal"
    );
}

/// The boundary family asks the opposite question to the near-equal one: not whether a verifier
/// notices a change too small to see, but whether it notices a change too large to be plausible.
/// A count at `i64::MAX`, a byte length of `-1`, an identifier past the exact-integer range of the
/// `f64` a JavaScript reader would parse it into — every one changes the canonical bytes, so every
/// one has to be refused, and none of them may be blamed on the digest.
#[test]
fn a_numeric_boundary_substitution_is_refused_at_every_numeric_position() {
    let mut cases_run = 0;
    let mut inexact_integers = 0;
    let mut numeric_positions = 0;
    let mut caught_by_the_digest_alone = 0;
    let mut caught_by_a_check_of_its_own = 0;
    for subject in subjects() {
        let here = subject
            .positions()
            .iter()
            .filter(|pointer| {
                matches!(
                    walk::get(&subject.document, pointer),
                    Some(Value::Number(_))
                )
            })
            .count();
        let cases = mutators::numeric_boundaries(&subject.document, &subject.positions());
        assert_eq!(
            cases.is_empty(),
            here == 0,
            "{} has {here} numeric positions and produced {} boundary cases",
            subject.label,
            cases.len()
        );
        numeric_positions += here;
        let family = subject.run(cases);
        family.assert_no_unexplained_hole();
        inexact_integers += family
            .cases
            .iter()
            .filter(|case| case.patch.value == serde_json::json!(mutators::FIRST_INEXACT_INTEGER))
            .count();
        for (outcome, count) in &family.run.outcomes {
            match *outcome {
                "digest_mismatch" => caught_by_the_digest_alone += count,
                "structural_failure" => caught_by_a_check_of_its_own += count,
                other => panic!(
                    "{}: a boundary substitution was answered with {other}",
                    subject.label
                ),
            }
        }
        cases_run += family.executed();
    }
    assert_eq!(
        (caught_by_the_digest_alone, caught_by_a_check_of_its_own),
        (1_891, 49),
        "the digest is carrying almost all of this. Only the delivery receipt, whose verifier \
         recomputes the whole projection from the delivery audit and compares it field by field, \
         notices an implausible number on its own; the four self-sealing documents apply no range \
         or plausibility check to any number they carry, so a consumer who reads one of them \
         without recomputing its digest is not protected from `i64::MAX` in a count at all"
    );
    assert_eq!(
        (cases_run, inexact_integers, numeric_positions),
        (1_940, 205, 205),
        "boundary cases across the five documents that carry a number at all — the delivery audit \
         is booleans, strings, and nulls, so it has no numeric position to substitute at — one of \
         them at every numeric position the first integer no f64 holds exactly"
    );
}

/// `NaN` and `Infinity` are not JSON. The battery says so rather than skipping them quietly: the
/// only form in which a non-finite number can reach a verifier through this format is a string
/// that spells one, which is what a coercing parser on the far side would revive it from.
#[test]
fn a_non_finite_number_can_only_reach_a_verifier_as_a_string_and_is_refused_as_one() {
    let mut checked = 0;
    for subject in subjects() {
        let cases: Vec<GeneratedCase> =
            mutators::numeric_boundaries(&subject.document, &subject.positions())
                .into_iter()
                .filter(|case| {
                    case.patch
                        .value
                        .as_str()
                        .is_some_and(|text| mutators::NON_FINITE_SPELLINGS.contains(&text))
                })
                .collect();
        assert_eq!(
            cases.len() % mutators::NON_FINITE_SPELLINGS.len(),
            0,
            "{}: every numeric position gets all three spellings",
            subject.label
        );
        let family = subject.run(cases);
        family.assert_no_unexplained_hole();
        checked += family.executed();
    }
    assert_eq!(checked, 615, "three spellings at every numeric position");
}

#[test]
fn replacing_any_visited_value_with_an_empty_string_or_null_is_rejected() {
    let mut cases_run = 0;
    for subject in subjects() {
        let cases = mutators::empty_or_null_substitutions(&subject.document, &subject.positions());
        assert!(
            !cases.is_empty(),
            "{} produced no substitution",
            subject.label
        );
        let family = subject.run(cases);
        family.assert_no_unexplained_hole();
        cases_run += family.executed();
    }
    assert_eq!(cases_run, 4_520, "empty-or-null cases across six documents");
}

#[test]
fn a_string_replaced_by_a_confusable_form_is_rejected_at_every_visited_position() {
    let mut cases_run = 0;
    for subject in subjects() {
        let family = subject.run(mutators::unicode_confusable_strings(
            &subject.document,
            &subject.positions(),
        ));
        family.assert_no_unexplained_hole();
        cases_run += family.executed();
    }
    assert_eq!(cases_run, 3_667, "confusable cases across six documents");
}

#[test]
fn a_swapped_pair_of_same_typed_siblings_is_rejected_at_every_visited_container() {
    let mut cases_run = 0;
    for subject in subjects() {
        let mut rng = SplitMix64::new(SEED);
        let cases = mutators::sibling_swaps(&subject.document, &subject.positions(), &mut rng);
        assert!(!cases.is_empty(), "{} produced no swap", subject.label);
        let family = subject.run(cases);
        family.assert_no_unexplained_hole();
        cases_run += family.executed();
    }
    assert_eq!(
        cases_run, 315,
        "sibling swaps across six documents — no key was added or removed, only the binding \
         between a name and a value"
    );
}

#[test]
fn an_unexpected_key_at_any_level_is_rejected_except_where_a_recorded_gap_says_otherwise() {
    let mut cases_run = 0;
    let mut excused = 0;
    for subject in subjects() {
        let mut rng = SplitMix64::new(SEED);
        let family = subject.run(mutators::unexpected_keys(
            &subject.document,
            &subject.positions(),
            &mut rng,
        ));
        family.assert_no_unexplained_hole();
        for hole in &family.run.holes {
            assert!(
                hole.observed.is_accepted(),
                "{}: the recorded gap says {} is accepted, and it was {} — delete the entry",
                subject.label,
                hole.description,
                hole.observed
            );
        }
        excused += family.excused();
        cases_run += family.executed();
    }
    assert_eq!(
        (cases_run, excused),
        (754, 2),
        "unexpected-key cases across six documents, two of them excused by a recorded gap"
    );
}

#[test]
fn an_object_written_with_a_duplicate_key_resolves_to_a_document_that_is_rejected() {
    let mut cases_run = 0;
    for subject in subjects() {
        let cases = mutators::wire_duplicate_keys(&subject.document, &subject.positions());
        assert!(!cases.is_empty(), "{} produced no duplicate", subject.label);
        let family = subject.run(cases);
        family.assert_no_unexplained_hole();
        cases_run += family.executed();
    }
    assert_eq!(cases_run, 376, "duplicate-key cases across six documents");
}

// -- cross-document confusion and idempotence -------------------------------------------------

#[test]
fn a_document_fed_to_a_verifier_that_does_not_own_it_is_always_rejected() {
    let delivery = documents::delivery_audit();
    let library: Vec<(&'static str, Value)> = vec![
        ("context_certificate", documents::certificate()),
        ("autopilot_report", documents::autopilot_report()),
        ("research_dossier", documents::dossier().clone()),
        ("mission_evidence_bundle", documents::evidence_bundle()),
        ("delivery_receipt", documents::delivery_receipt()),
    ];
    let mut confusions = 0;
    for (verifier_name, verify) in verifiers::all(&delivery) {
        for (document_name, document) in &library {
            let verdict = verify(document);
            if document_name == &verifier_name {
                assert_eq!(
                    verdict,
                    Verdict::Accepted,
                    "{verifier_name} must accept its own document"
                );
                continue;
            }
            assert!(
                !verdict.is_accepted(),
                "{verifier_name} accepted a {document_name}"
            );
            confusions += 1;
        }
    }
    assert_eq!(
        confusions, 20,
        "five verifiers against four foreign documents each"
    );
}

#[test]
fn verifying_the_same_document_twice_yields_byte_identical_projections() {
    let certificate = documents::certificate();
    assert_eq!(
        ContextCertificate::verify(&certificate).expect("verification runs"),
        ContextCertificate::verify(&certificate).expect("verification runs")
    );

    let projections: Vec<(&str, Value, Value)> = vec![
        (
            "autopilot_report",
            bioprism_autopilot::verify_autopilot_report(&documents::autopilot_report())
                .expect("verification runs"),
            bioprism_autopilot::verify_autopilot_report(&documents::autopilot_report())
                .expect("verification runs"),
        ),
        (
            "research_dossier",
            bioprism_research::verify_dossier(documents::dossier()).expect("verification runs"),
            bioprism_research::verify_dossier(documents::dossier()).expect("verification runs"),
        ),
        (
            "mission_evidence_bundle",
            bioprism_devplat::verify_mission_evidence_bundle(&documents::evidence_bundle())
                .expect("verification runs"),
            bioprism_devplat::verify_mission_evidence_bundle(&documents::evidence_bundle())
                .expect("verification runs"),
        ),
    ];
    for (label, first, second) in projections {
        assert_eq!(
            to_canonical_string(&first).expect("canonicalises"),
            to_canonical_string(&second).expect("canonicalises"),
            "{label} is not idempotent"
        );
    }

    let delivery = documents::delivery_audit();
    let receipt = documents::delivery_receipt();
    let first = verifiers::delivery_receipt(&receipt, &delivery);
    let second = verifiers::delivery_receipt(&receipt, &delivery);
    assert_eq!(first, second);

    // Idempotence has to survive rejection too: a verifier that memoised its first answer would
    // pass the checks above and still report a tampered document differently the second time.
    let tampered = wrong_digest(&documents::autopilot_report(), "/report_sha256");
    let first = bioprism_autopilot::verify_autopilot_report(&tampered).expect("verification runs");
    let second = bioprism_autopilot::verify_autopilot_report(&tampered).expect("verification runs");
    assert_eq!(
        to_canonical_string(&first).expect("canonicalises"),
        to_canonical_string(&second).expect("canonicalises")
    );
}

// -- differential digest paths ----------------------------------------------------------------

#[test]
fn the_certificate_digest_agrees_across_the_struct_the_document_and_a_direct_sha256() {
    for profile in [CertificateProfile::Reference, CertificateProfile::Extended] {
        let document = match profile {
            CertificateProfile::Reference => documents::certificate(),
            CertificateProfile::Extended => documents::extended_certificate(),
        };
        let embedded = document["certificate_sha256"]
            .as_str()
            .expect("the certificate carries its digest");

        let mut body = document.as_object().expect("object").clone();
        body.remove("certificate_sha256");
        let body = Value::Object(body);

        let through_content_hash = ContentHash::of_value(&body)
            .expect("canonicalises")
            .to_string();
        let direct = {
            let mut hasher = Sha256::new();
            hasher.update(
                to_canonical_string(&body)
                    .expect("canonicalises")
                    .as_bytes(),
            );
            hasher
                .finalize()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        };

        assert_eq!(embedded, through_content_hash, "{profile:?}");
        assert_eq!(
            embedded, direct,
            "{profile:?}: the embedded digest must be sha256 over the canonical bytes and nothing \
             else — a wrapper that pre-processed the input would show up here"
        );
    }

    let reference = documents::certificate();
    let extended = documents::extended_certificate();
    assert_ne!(
        reference["certificate_sha256"], extended["certificate_sha256"],
        "two profiles that hash different field sets must not share an identity"
    );
}

#[test]
fn every_inlined_dossier_artifact_hashes_to_the_digest_its_record_claims() {
    let dossier = documents::dossier();
    let mut checked = 0;
    for step in dossier["steps"].as_array().expect("steps") {
        for output in step["outputs"].as_array().expect("outputs") {
            let claimed = output["sha256"]
                .as_str()
                .expect("a record carries a digest");
            if output["inlined"] != Value::Bool(true) {
                continue;
            }
            let artifact = &output["artifact"];
            assert_eq!(
                ContentHash::of_value(artifact)
                    .expect("canonicalises")
                    .as_str(),
                claimed,
                "artifact {} records a digest its content does not produce",
                output["name"]
            );
            assert_eq!(
                to_canonical_string(artifact).expect("canonicalises").len(),
                output["canonical_bytes"].as_u64().expect("a byte count") as usize,
                "artifact {} records a byte count its content does not produce",
                output["name"]
            );
            checked += 1;
        }
    }
    assert!(
        checked >= 5,
        "only {checked} inlined artifacts were checkable"
    );
}

#[test]
fn the_figure_renderer_and_the_dossier_record_agree_on_every_artifact_digest() {
    let dossier = documents::dossier();
    let rendered = render_report(dossier).expect("the report renders");
    let recorded: Vec<String> = dossier["steps"]
        .as_array()
        .expect("steps")
        .iter()
        .flat_map(|step| step["outputs"].as_array().expect("outputs"))
        .filter_map(|output| output["sha256"].as_str().map(str::to_string))
        .collect();

    assert!(!rendered.figures.is_empty(), "the report renders no figure");
    let mut matched = 0;
    for (filename, svg) in &rendered.figures {
        let footer = svg
            .split("source sha256: ")
            .nth(1)
            .and_then(|tail| tail.split('<').next())
            .expect("every figure footer carries its source digest");
        let footer = footer.trim();
        assert!(
            recorded.iter().any(|digest| digest == footer),
            "figure {filename} was rendered from {footer}, which no dossier record claims — the \
             renderer and the dossier disagree about what was drawn"
        );
        matched += 1;
    }
    assert_eq!(matched, rendered.figures.len());
}

#[test]
fn the_dossier_digest_the_verifier_recomputes_is_the_one_the_rendered_report_prints() {
    let dossier = documents::dossier();
    let projection = bioprism_research::verify_dossier(dossier).expect("verification runs");
    let recomputed = projection["recomputed_dossier_sha256"]
        .as_str()
        .expect("the projection carries the recomputed digest");
    let report = render_report(dossier)
        .expect("the report renders")
        .report_md;
    assert!(
        report.contains(&format!("dossier digest: `{recomputed}`")),
        "the rendered report must cite the digest the verifier recomputes, not a separately \
         derived one"
    );
    assert_eq!(dossier["dossier_sha256"].as_str(), Some(recomputed));
}

// -- the adapters' assumptions ----------------------------------------------------------------

#[test]
fn the_rejection_reason_strings_the_adapters_key_on_are_the_ones_the_verifiers_emit() {
    let stripped =
        walk::with_removal(&documents::certificate(), "/certificate_sha256").expect("removable");
    let bioprism_section::CertificateVerification::Malformed(reason) =
        ContextCertificate::verify(&stripped).expect("verification runs")
    else {
        panic!("a certificate without its digest is malformed");
    };
    assert!(
        reason.contains(verifiers::CERTIFICATE_ABSENT_DIGEST),
        "{reason}"
    );

    let broken = walk::with_replacement(
        &documents::certificate(),
        "/certificate_sha256",
        Value::String("nope".into()),
    )
    .expect("replaceable");
    let bioprism_section::CertificateVerification::Malformed(reason) =
        ContextCertificate::verify(&broken).expect("verification runs")
    else {
        panic!("a shape-broken digest is malformed, not a mismatch");
    };
    assert!(
        reason.contains(verifiers::CERTIFICATE_MALFORMED_DIGEST),
        "{reason}"
    );

    let stripped =
        walk::with_removal(&documents::autopilot_report(), "/report_sha256").expect("removable");
    let error = bioprism_autopilot::verify_autopilot_report(&stripped)
        .expect_err("a report without its digest is refused");
    assert!(
        error
            .to_string()
            .contains(verifiers::AUTOPILOT_ABSENT_DIGEST),
        "{error}"
    );

    let stripped = walk::with_removal(documents::dossier(), "/dossier_sha256").expect("removable");
    let error = bioprism_research::verify_dossier(&stripped)
        .expect_err("a dossier without its digest is refused");
    assert!(
        error.to_string().contains(verifiers::DOSSIER_ABSENT_DIGEST),
        "{error}"
    );

    let stripped =
        walk::with_removal(&documents::evidence_bundle(), "/bundle_digest").expect("removable");
    let error = bioprism_devplat::verify_mission_evidence_bundle(&stripped)
        .expect_err("a bundle without its digest is refused");
    assert!(
        error.to_string().contains(verifiers::BUNDLE_ABSENT_DIGEST),
        "{error}"
    );

    let broken = walk::with_replacement(
        &documents::evidence_bundle(),
        "/bundle_digest",
        Value::String("nope".into()),
    )
    .expect("replaceable");
    let error = bioprism_devplat::verify_mission_evidence_bundle(&broken)
        .expect_err("a shape-broken bundle digest is refused");
    assert!(
        error
            .to_string()
            .contains(verifiers::BUNDLE_MALFORMED_DIGEST),
        "{error}"
    );
}
