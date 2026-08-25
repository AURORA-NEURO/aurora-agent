//! The depth battery: every generator, at every position, against every digest-sealed document
//! type the workspace ships a verifier for.
//!
//! Each test states one property and reports what it measured. The counts in the assertions are
//! pinned rather than bounded from below on purpose: a coverage number that can silently shrink
//! is not a coverage number, and a generator that stops producing cases would otherwise turn this
//! file green by doing nothing.

mod documents;
mod verifiers;

use bioprism_ids::{to_canonical_string, ContentHash};
use bioprism_receipts_audit::{
    mutators, run_battery, rng::SplitMix64, walk, BatteryConfig, Expect, RejectionClass, Verdict,
};
use bioprism_research::render_report;
use bioprism_section::{CertificateProfile, ContextCertificate};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// One seed for the whole battery. Every failure message repeats it, so a reported hole is a
/// complete reproduction recipe: this seed, that document type, that pointer.
const SEED: u64 = 0x9E37_79B9_7F4A_7C15;

/// The structural families visit every position of every document except the research dossier,
/// which has 1981 of them. Visiting all of them costs minutes of hashing for coverage the strided
/// selection already provides, so the dossier is bounded — deterministically, to every eighth
/// pointer in document order — and every assertion that touches it says so.
const DOSSIER_POSITION_CAP: usize = 250;

struct Subject {
    label: &'static str,
    document: Value,
    verify: Box<dyn Fn(&Value) -> Verdict>,
    config: BatteryConfig,
    /// The field whose value seals the document, where it has one.
    sealing_digest: Option<&'static str>,
}

impl Subject {
    fn positions(&self) -> Vec<String> {
        walk::strided(&walk::pointers(&self.document), self.config.position_cap).0
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
            config: BatteryConfig::exhaustive("context_certificate", SEED),
            sealing_digest: Some("/certificate_sha256"),
        },
        Subject {
            label: "autopilot_report",
            document: documents::autopilot_report(),
            verify: Box::new(verifiers::autopilot),
            config: BatteryConfig::exhaustive("autopilot_report", SEED),
            sealing_digest: Some("/report_sha256"),
        },
        Subject {
            label: "research_dossier",
            document: documents::dossier().clone(),
            verify: Box::new(verifiers::dossier),
            config: BatteryConfig::bounded("research_dossier", SEED, DOSSIER_POSITION_CAP),
            sealing_digest: Some("/dossier_sha256"),
        },
        Subject {
            label: "mission_evidence_bundle",
            document: documents::evidence_bundle(),
            verify: Box::new(verifiers::evidence_bundle),
            config: BatteryConfig::exhaustive("mission_evidence_bundle", SEED),
            sealing_digest: Some("/bundle_digest"),
        },
        Subject {
            label: "delivery_receipt",
            document: receipt,
            verify: Box::new(move |document| verifiers::delivery_receipt(document, &delivery)),
            config: BatteryConfig::exhaustive("delivery_receipt", SEED),
            sealing_digest: Some("/receipt_digest"),
        },
        Subject {
            label: "delivery_audit_behind_a_fixed_receipt",
            document: documents::delivery_audit(),
            verify: Box::new(move |document| {
                verifiers::delivery_receipt(&receipt_for_audit, document)
            }),
            config: BatteryConfig::exhaustive("delivery_audit_behind_a_fixed_receipt", SEED),
            sealing_digest: None,
        },
    ]
}

/// A case the battery generates, a verifier accepts, and this repository has decided not to
/// close — recorded here so the exemption is visible, justified, and load-bearing.
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
        (total_cases, total_positions),
        (6_527, 542),
        "the battery's coverage is a pinned claim; bounds were:\n{}",
        bounds.join("\n")
    );
}

// -- digest integrity, exhaustively -----------------------------------------------------------

#[test]
fn every_single_byte_digest_mutation_is_caught_at_every_offset_of_every_digest_field() {
    let mut fields = 0;
    let mut offsets = 0;
    for subject in subjects() {
        let found = mutators::digest_pointers(&subject.document);
        let mut rng = SplitMix64::new(SEED);
        let cases = mutators::digest_byte_flips(&subject.document, &mut rng);
        assert_eq!(
            cases.len(),
            found.len() * mutators::DIGEST_CHARS,
            "{}: digest coverage must be exhaustive over offsets, never sampled",
            subject.label
        );
        for case in &cases {
            let verdict = (subject.verify)(&case.document);
            assert!(
                !verdict.is_accepted(),
                "{} (seed {SEED}): {} verified anyway",
                subject.label,
                case.description
            );
        }
        fields += found.len();
        offsets += cases.len();
    }
    assert_eq!(
        (fields, offsets),
        (51, 3_264),
        "51 digest fields across six documents, each checked at all 64 offsets"
    );
}

#[test]
fn a_truncated_extended_or_recased_digest_is_caught_at_every_digest_field() {
    let mut cases_run = 0;
    for subject in subjects() {
        let mut cases = mutators::digest_length_changes(&subject.document);
        cases.extend(mutators::digest_case_changes(&subject.document));
        for case in &cases {
            let verdict = (subject.verify)(&case.document);
            assert!(
                !verdict.is_accepted(),
                "{} (seed {SEED}): {} verified anyway",
                subject.label,
                case.description
            );
        }
        cases_run += cases.len();
    }
    assert_eq!(cases_run, 357, "51 digest fields, seven shape mutations each");
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
        assert!(!cases.is_empty(), "{} produced no reordering", subject.label);
        for case in &cases {
            assert_eq!(case.expect, Expect::VerdictUnchanged);
            assert_eq!(
                to_canonical_string(&case.document).expect("canonicalises"),
                baseline_bytes,
                "{} (seed {SEED}): {} moved the canonical bytes — canonicalisation is not \
                 key-order invariant, and every digest in this workspace is an artefact of one \
                 serializer",
                subject.label,
                case.description
            );
            assert_eq!(
                (subject.verify)(&case.document),
                baseline,
                "{} (seed {SEED}): {} changed the verdict",
                subject.label,
                case.description
            );
        }
        cases_run += cases.len();
    }
    assert_eq!(cases_run, 114, "reordering cases across six documents");
}

#[test]
fn array_reordering_always_changes_a_verdict_at_any_position() {
    let mut cases_run = 0;
    for subject in subjects() {
        let mut rng = SplitMix64::new(SEED);
        let cases = mutators::array_reorderings(&subject.document, &subject.positions(), &mut rng);
        for case in &cases {
            assert!(
                !(subject.verify)(&case.document).is_accepted(),
                "{} (seed {SEED}): {} verified anyway — JSON arrays are ordered and a digest that \
                 ignores their order is not naming the document",
                subject.label,
                case.description
            );
        }
        cases_run += cases.len();
    }
    assert_eq!(cases_run, 44, "array reordering cases across six documents");
}

// -- absent, malformed, and mismatching digests stay three different answers -------------------

#[test]
fn a_document_whose_sealing_digest_is_absent_is_rejected_distinctly_from_one_whose_digest_is_wrong()
{
    let mut checked = 0;
    for subject in subjects() {
        let Some(pointer) = subject.sealing_digest else {
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
        let Some(pointer) = subject.sealing_digest else {
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
        for case in &cases {
            let verdict = (subject.verify)(&case.document);
            assert!(
                !verdict.is_accepted(),
                "{} (seed {SEED}): {} verified anyway",
                subject.label,
                case.description
            );
        }
        cases_run += cases.len();
    }
    assert_eq!(cases_run, 525, "deletion cases across six documents");
}

// -- numbers, strings, and structure ----------------------------------------------------------

#[test]
fn a_numeric_near_equal_substitution_lands_on_one_stable_verdict_and_never_between_them() {
    let mut cases_run = 0;
    for subject in subjects() {
        let cases = mutators::numeric_near_equal(&subject.document, &subject.positions());
        for case in &cases {
            let first = (subject.verify)(&case.document);
            let second = (subject.verify)(&case.document);
            assert_eq!(
                first, second,
                "{} (seed {SEED}): {} answered differently on two runs",
                subject.label, case.description
            );
            assert!(
                !first.is_accepted(),
                "{} (seed {SEED}): {} verified anyway — an integer and its equal-valued float are \
                 different canonical bytes, so a verifier that accepted both would make its digest \
                 depend on how a caller's parser typed a literal",
                subject.label,
                case.description
            );
        }
        cases_run += cases.len();
    }
    assert_eq!(cases_run, 65, "numeric cases across six documents");
}

#[test]
fn replacing_any_visited_value_with_an_empty_string_or_null_is_rejected() {
    let mut cases_run = 0;
    for subject in subjects() {
        let cases =
            mutators::empty_or_null_substitutions(&subject.document, &subject.positions());
        assert!(!cases.is_empty(), "{} produced no substitution", subject.label);
        for case in &cases {
            assert!(
                !(subject.verify)(&case.document).is_accepted(),
                "{} (seed {SEED}): {} verified anyway",
                subject.label,
                case.description
            );
        }
        cases_run += cases.len();
    }
    assert_eq!(cases_run, 1_055, "empty-or-null cases across six documents");
}

#[test]
fn a_string_replaced_by_a_confusable_form_is_rejected_at_every_visited_position() {
    let mut cases_run = 0;
    for subject in subjects() {
        let cases = mutators::unicode_confusable_strings(&subject.document, &subject.positions());
        for case in &cases {
            assert!(
                !(subject.verify)(&case.document).is_accepted(),
                "{} (seed {SEED}): {} verified anyway",
                subject.label,
                case.description
            );
        }
        cases_run += cases.len();
    }
    assert_eq!(cases_run, 746, "confusable cases across six documents");
}

#[test]
fn a_swapped_pair_of_same_typed_siblings_is_rejected_at_every_visited_container() {
    let mut cases_run = 0;
    for subject in subjects() {
        let mut rng = SplitMix64::new(SEED);
        let cases = mutators::sibling_swaps(&subject.document, &subject.positions(), &mut rng);
        assert!(!cases.is_empty(), "{} produced no swap", subject.label);
        for case in &cases {
            assert!(
                !(subject.verify)(&case.document).is_accepted(),
                "{} (seed {SEED}): {} verified anyway — no key was added or removed, only the \
                 binding between a name and a value",
                subject.label,
                case.description
            );
        }
        cases_run += cases.len();
    }
    assert_eq!(cases_run, 72, "sibling swaps across six documents");
}

#[test]
fn an_unexpected_key_at_any_level_is_rejected_except_where_a_recorded_gap_says_otherwise() {
    let mut cases_run = 0;
    let mut excused = 0;
    for subject in subjects() {
        let mut rng = SplitMix64::new(SEED);
        let cases = mutators::unexpected_keys(&subject.document, &subject.positions(), &mut rng);
        for case in &cases {
            let excused_here = KNOWN_GAPS
                .iter()
                .any(|gap| gap.matches(subject.label, case.mutator, &case.pointer));
            let accepted = (subject.verify)(&case.document).is_accepted();
            if excused_here {
                assert!(
                    accepted,
                    "{}: the recorded gap says {} is accepted, and it was not — delete the entry",
                    subject.label, case.description
                );
                excused += 1;
                continue;
            }
            assert!(
                !accepted,
                "{} (seed {SEED}): {} verified anyway",
                subject.label,
                case.description
            );
        }
        cases_run += cases.len();
    }
    assert_eq!(
        (cases_run, excused),
        (190, 2),
        "unexpected-key cases across six documents, two of them excused by a recorded gap"
    );
}

#[test]
fn an_object_written_with_a_duplicate_key_resolves_to_a_document_that_is_rejected() {
    let mut cases_run = 0;
    for subject in subjects() {
        let cases = mutators::wire_duplicate_keys(&subject.document, &subject.positions());
        assert!(!cases.is_empty(), "{} produced no duplicate", subject.label);
        for case in &cases {
            assert!(
                !(subject.verify)(&case.document).is_accepted(),
                "{} (seed {SEED}): {} verified anyway",
                subject.label,
                case.description
            );
        }
        cases_run += cases.len();
    }
    assert_eq!(cases_run, 95, "duplicate-key cases across six documents");
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
    assert_eq!(confusions, 20, "five verifiers against four foreign documents each");
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
            hasher.update(to_canonical_string(&body).expect("canonicalises").as_bytes());
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
            let claimed = output["sha256"].as_str().expect("a record carries a digest");
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
    assert!(checked >= 5, "only {checked} inlined artifacts were checkable");
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
    let report = render_report(dossier).expect("the report renders").report_md;
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
    assert!(reason.contains(verifiers::CERTIFICATE_ABSENT_DIGEST), "{reason}");

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
        error.to_string().contains(verifiers::AUTOPILOT_ABSENT_DIGEST),
        "{error}"
    );

    let stripped = walk::with_removal(documents::dossier(), "/dossier_sha256").expect("removable");
    let error =
        bioprism_research::verify_dossier(&stripped).expect_err("a dossier without its digest is refused");
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
        error.to_string().contains(verifiers::BUNDLE_MALFORMED_DIGEST),
        "{error}"
    );
}
