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
//! # What the library gives a test
//!
//! - [`rng::SplitMix64`], so a reported failure carries a seed that regenerates it exactly.
//! - [`walk`], which enumerates every JSON pointer in a document and edits at one.
//! - [`mutators`], the generators, each declaring [`mutators::Expect`] for what it produces.
//! - [`run_battery`], which drives all of it against a verifier expressed as a
//!   [`Verdict`]-returning closure and reports [`Coverage`] alongside any [`Hole`].
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

use bioprism_ids::to_canonical_string;
use mutators::{Mutation, MUTATORS};
use rng::SplitMix64;
use serde_json::Value;
use std::fmt;

pub use mutators::{Expect, Mutation as GeneratedCase};

/// Why a verifier refused a document.
///
/// The classes are separate because the distinction between them is a shipped feature, not an
/// implementation detail: a digest that is the wrong *shape* is a defect in the claimed digest,
/// and a digest that is the right shape and the wrong *value* is evidence that the body moved.
/// Reporting the first as the second would accuse a caller of tampering on the strength of a typo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
                "{}: exhaustive over all {} positions, {} digest fields, {} digest offsets, {} cases (seed {})",
                self.label,
                self.positions_total,
                self.digest_fields,
                self.digest_offsets_covered,
                self.cases,
                self.seed
            )
        } else {
            format!(
                "{}: BOUNDED to every {}th JSON pointer in document order, {} of {} positions; digest coverage stays exhaustive at {} fields and {} offsets; {} cases (seed {})",
                self.label,
                self.position_step,
                self.positions_covered,
                self.positions_total,
                self.digest_fields,
                self.digest_offsets_covered,
                self.cases,
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
            self.expect.as_str(),
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

    /// The multi-line message a failing test prints: the bound, then every hole with its seed.
    pub fn report(&self) -> String {
        let mut lines = vec![self.coverage.bound_statement()];
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

/// How wide a battery run is allowed to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatteryConfig {
    /// Names the document type in every failure message.
    pub label: &'static str,
    pub seed: u64,
    /// Maximum positions the structural families may visit. `0` means no bound. Digest coverage
    /// is never bounded by this.
    pub position_cap: usize,
}

impl BatteryConfig {
    pub fn exhaustive(label: &'static str, seed: u64) -> Self {
        BatteryConfig {
            label,
            seed,
            position_cap: 0,
        }
    }

    pub fn bounded(label: &'static str, seed: u64, position_cap: usize) -> Self {
        BatteryConfig {
            label,
            seed,
            position_cap,
        }
    }
}

fn tally(cases: &[Mutation]) -> Vec<(&'static str, usize)> {
    MUTATORS
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
    let baseline = verify(document);
    let baseline_canonical = to_canonical_string(document).ok();

    let all_positions = walk::pointers(document);
    let (positions, position_step) = walk::strided(&all_positions, config.position_cap);
    let digest_fields = mutators::digest_pointers(document);

    let mut rng = SplitMix64::new(config.seed);
    let (cases, degenerate_dropped) = mutators::generate(document, &positions, &mut rng);

    let mut holes = Vec::new();
    let mut canonicalisation_violations = Vec::new();
    for case in &cases {
        let observed = verify(&case.document);
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
                if to_canonical_string(&case.document).ok() != baseline_canonical {
                    canonicalisation_violations.push(hole(observed.clone()));
                }
                if observed != baseline {
                    holes.push(hole(observed));
                }
            }
            Expect::Rejected => {
                if observed.is_accepted() {
                    holes.push(hole(observed));
                }
            }
        }
    }

    BatteryOutcome {
        coverage: Coverage {
            label: config.label,
            seed: config.seed,
            positions_total: all_positions.len(),
            positions_covered: positions.len(),
            position_step,
            digest_fields: digest_fields.len(),
            digest_offsets_covered: cases
                .iter()
                .filter(|case| case.mutator == "digest_byte_flip")
                .count(),
            cases: cases.len(),
            cases_by_mutator: tally(&cases),
            degenerate_dropped,
        },
        baseline,
        holes,
        canonicalisation_violations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_ids::ContentHash;
    use serde_json::json;

    fn sealed() -> Value {
        let mut document = json!({
            "schema": "audit/self-test/0.1",
            "rows": [{ "id": "one", "weight": 1 }, { "id": "two", "weight": 2 }],
            "note": "a sealed document",
        });
        let digest = ContentHash::of_value(&document).expect("canonicalises").to_string();
        document["sha256"] = Value::String(digest);
        document
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

    /// The same verifier with the flaw the battery exists to catch: it hashes only the keys it
    /// happens to know about, so anything added or moved elsewhere passes.
    fn shallow(document: &Value) -> Verdict {
        let Some(object) = document.as_object() else {
            return Verdict::rejected(RejectionClass::Malformed, "not an object");
        };
        let Some(claimed) = object.get("sha256").and_then(Value::as_str) else {
            return Verdict::rejected(RejectionClass::DigestAbsent, "no sha256");
        };
        let covered = json!({ "schema": object.get("schema"), "note": object.get("note") });
        match ContentHash::of_value(&covered) {
            Ok(_) if claimed.len() == 64 => Verdict::Accepted,
            _ => Verdict::rejected(RejectionClass::Malformed, "unhashable"),
        }
    }

    #[test]
    fn an_honest_verifier_survives_the_whole_battery() {
        let outcome = run_battery(
            &sealed(),
            &BatteryConfig::exhaustive("self-test", 0xA11CE),
            &honest,
        );
        assert!(outcome.is_clean(), "{}", outcome.report());
        assert_eq!(outcome.baseline, Verdict::Accepted);
        assert!(outcome.coverage.cases > 100, "{}", outcome.coverage);
    }

    #[test]
    fn a_verifier_that_hashes_only_the_keys_it_knows_is_caught_at_many_positions() {
        let outcome = run_battery(
            &sealed(),
            &BatteryConfig::exhaustive("self-test", 0xA11CE),
            &shallow,
        );
        assert!(!outcome.is_clean());
        assert!(
            outcome.holes.len() > 20,
            "a shallow verifier must fail widely, not once: {}",
            outcome.report()
        );
        assert!(outcome
            .holes
            .iter()
            .any(|hole| hole.mutator == "array_reordering"));
        assert!(outcome
            .holes
            .iter()
            .any(|hole| hole.mutator == "sibling_swap"));
    }

    #[test]
    fn digest_coverage_stays_exhaustive_when_the_position_budget_is_bound() {
        let document = sealed();
        let bounded = run_battery(
            &document,
            &BatteryConfig::bounded("self-test", 7, 3),
            &honest,
        );
        assert!(!bounded.coverage.is_exhaustive());
        assert!(bounded.coverage.positions_covered < bounded.coverage.positions_total);
        assert!(bounded.coverage.bound_statement().contains("BOUNDED"));

        let exhaustive = run_battery(&document, &BatteryConfig::exhaustive("self-test", 7), &honest);
        assert_eq!(
            bounded.coverage.digest_offsets_covered,
            exhaustive.coverage.digest_offsets_covered
        );
        assert_eq!(bounded.coverage.digest_offsets_covered, mutators::DIGEST_CHARS);
    }

    #[test]
    fn the_same_seed_reruns_the_same_cases_and_reports_the_same_coverage() {
        let document = sealed();
        let config = BatteryConfig::exhaustive("self-test", 424242);
        let first = run_battery(&document, &config, &honest);
        let second = run_battery(&document, &config, &honest);
        assert_eq!(first, second);
    }

    #[test]
    fn a_baseline_the_verifier_already_refuses_is_reported_rather_than_measured() {
        let mut broken = sealed();
        broken["note"] = json!("edited after sealing");
        let outcome = run_battery(
            &broken,
            &BatteryConfig::exhaustive("self-test", 1),
            &honest,
        );
        assert!(!outcome.is_clean());
        assert_eq!(outcome.baseline.class(), Some(RejectionClass::DigestMismatch));
        assert!(outcome.report().contains("the run measured nothing"));
    }
}
