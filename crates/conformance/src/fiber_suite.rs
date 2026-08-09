//! The FIBER compiler conformance suite.
//!
//! This is the concrete instance of blueprint 40.32: a versioned set of requirements any
//! implementation claiming to be a FIBER query compiler must satisfy, expressed as data so that a
//! CPython or TypeScript compiler can be certified against exactly the cases the Rust engine was.
//! [`FiberReference`] is one implementation under test, not the definition of correct — the cases
//! are.
//!
//! Every case names the blueprint module it enforces. That is not bookkeeping: a requirement with
//! no source is somebody's preference, and the first time it inconveniences a vendor there is no
//! argument for keeping it.
//!
//! ## What the suite covers
//!
//! The properties 43.41 and 43.13 make non-negotiable, in the order they matter: protected
//! closure survives compilation; the certificate's digest recomputes; the certificate binds the
//! section actually delivered; a budget below the protected closure refuses rather than trimming
//! evidence; omissions are counted and influence-classified rather than hidden; passes the engine
//! cannot run are declared rather than silently skipped; compilation is deterministic.
//!
//! ## What it does not cover
//!
//! Everything the shipped world cannot separate. The reference world is a single synthetic
//! radiogenomic case in which a graph walk and a lexical retriever select the same eleven facts
//! (`docs/FINDINGS.md`), so passing this suite says an implementation reproduces the reference
//! contract — it says nothing about whether the compiler is better than a keyword search on any
//! other world. 40.32 invariant 1 states this generally: conformance is not accuracy. Structural
//! discrimination belongs to `bioprism-worldgen` and `bioprism-baseline`, not here.
//!
//! No case exercises a plan fallback, an underdetermined oracle verdict, a policy-blocked
//! omission or a world whose temporal cut actually withholds a selected fact, because the shipped
//! fixture triggers none of them. Those are gaps in the fixture set, and 40.33 would have them
//! filled by negative variants rather than left to an implementer to guess at. Being able to add
//! a key to a fixture does not shorten that list: not one of the four turns on a key the baseline
//! is missing, so [`crate::case::OverrideOp::Insert`] leaves all four exactly where they were.
//!
//! The refusals are the other half of the same accounting. [`fiber_failure`], [`world_failure`]
//! and [`policy_failure`] between them name twenty-three failure kinds, of which five have cases:
//! `budget_exceeded`, `unsupported_query_schema`, `unsupported_world_schema`,
//! `invalid_identifier`, and — since the query format closed its key set — `undeclared_query_field`,
//! exercised both at the top level and inside `budgets`. The other eighteen are declared in the
//! taxonomy and unexercised here, and one of them is not merely unwritten but currently
//! *inexpressible*: `missing_query_field` needs a variant with a key removed, and an override can
//! change or add a key but never delete one (see [`crate::Override`]). That one needs a fixture
//! too.

use crate::case::{CaseBuilder, CaseInput, ConformanceCase, Expectation, Layer};
use crate::fixture::{FixtureCard, FixtureManifest, FixtureRole, FixtureShape};
use crate::gate::BaselineReference;
use crate::implementation::{
    artifact, CompileArtifacts, CompileFailure, Implementation, ImplementationIdentity,
};
use crate::suite::Suite;
use bioprism_fiber::{compile, CompileOutput, FiberError, PolicyViolation, Query};
use bioprism_section::CertificateProfile;
use bioprism_world::{World, WorldError};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub const FIBER_SUITE_ID: &str = "fiber-compiler-conformance";
pub const FIBER_SUITE_VERSION: &str = "0.1.0";
pub const FIBER_REPORT_SCHEMA_VERSION: &str = "fiber-compiler-report/0.1";

pub const WORLD_FIXTURE: &str = "fiber.world.radiogenomic";
pub const QUERY_FIXTURE: &str = "fiber.query.leakage";
pub const GOLDEN_SECTION_FIXTURE: &str = "fiber.golden.section";
pub const GOLDEN_CERTIFICATE_FIXTURE: &str = "fiber.golden.certificate";

/// The published Reference-profile certificate digest for the golden pair.
pub const REFERENCE_CERTIFICATE_SHA256: &str =
    "c0da17ffc80465258345c8a538171bfd868100cd883e9a20780a0dc5477e7ea4";
/// The published Decision Section digest for the golden pair.
pub const REFERENCE_SECTION_SHA256: &str =
    "7439b2262c52c1c794b59be86d922b723a2ea5646362d529f57fb11b5f7e93ce";
pub const REFERENCE_WORLD_SHA256: &str =
    "b3809731cf93040fcd8aef43deb2a552492064b49154e07ea58caa724c10cbb5";
pub const REFERENCE_QUERY_SHA256: &str =
    "d06368c62dd878f483d30ebfd56f38426b1234d37e46ca77d4536db81f59c684";

/// The eleven protected facts whose loss would make the split-integrity verdict unsupportable.
const PROTECTED_FACTS: [&str; 11] = [
    "fact.cohort",
    "fact.decision_cut",
    "fact.label_source",
    "fact.negative_duplicates",
    "fact.policy",
    "fact.preprocess_fit",
    "fact.scanner",
    "fact.site",
    "fact.specimen_dates",
    "fact.split",
    "fact.subject_aliases",
];

/// Where the repository keeps its fixtures, relative to this crate.
pub fn workspace_fixture_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", "..", "fixtures"]
        .iter()
        .collect()
}

/// The fixture cards for `fiber-v0.1`.
///
/// Digests are over canonical values, so the declared hash of the world equals the
/// `source_hashes.world_sha256` a compile records — the same number, checked twice from opposite
/// directions.
pub fn fiber_fixture_manifest() -> FixtureManifest {
    FixtureManifest::new(
        "fiber-v0.1",
        vec![
            FixtureCard {
                id: WORLD_FIXTURE.to_string(),
                path: "fiber-v0.1/radiogenomic_world.json".to_string(),
                role: FixtureRole::World,
                sha256: REFERENCE_WORLD_SHA256.to_string(),
                provenance: "authored for the 43.42 vertical slice; no patient data of any kind"
                    .to_string(),
                license: "Apache-2.0, distributed with the repository".to_string(),
                synthetic: true,
                shape: shape(
                    &[
                        "description",
                        "events",
                        "factors",
                        "facts",
                        "schema_version",
                        "world_id",
                    ],
                    332_026,
                    &[("events", 2), ("factors", 756), ("facts", 761)],
                ),
                notes: "761 facts, of which 750 are exploratory distractors attached to a single \
                        hub variable. Synthetic and deliberately unrealistic: it exists to make \
                        four leakage mechanisms simultaneously present and detectable, not to \
                        resemble a cohort."
                    .to_string(),
            },
            FixtureCard {
                id: QUERY_FIXTURE.to_string(),
                path: "fiber-v0.1/leakage_query.json".to_string(),
                role: FixtureRole::Query,
                sha256: REFERENCE_QUERY_SHA256.to_string(),
                provenance: "authored for the 43.42 vertical slice".to_string(),
                license: "Apache-2.0, distributed with the repository".to_string(),
                synthetic: true,
                shape: shape(
                    &[
                        "budgets",
                        "decision_time",
                        "distortion_tolerance",
                        "policy",
                        "protected_tags",
                        "query_id",
                        "role",
                        "schema_version",
                        "targets",
                    ],
                    405,
                    &[
                        ("budgets", 2),
                        ("policy", 1),
                        ("protected_tags", 10),
                        ("targets", 1),
                    ],
                ),
                notes: "One target, ten protected tags, a generous fact budget. Negative variants \
                        are expressed as pointer overrides on the cases rather than as separate \
                        files."
                    .to_string(),
            },
            FixtureCard {
                id: GOLDEN_SECTION_FIXTURE.to_string(),
                path: "fiber-v0.1/golden/reference_section.json".to_string(),
                role: FixtureRole::GoldenOutput,
                sha256: REFERENCE_SECTION_SHA256.to_string(),
                provenance: "emitted by reference/fiber_runtime/fiber_compile.py, an independent \
                             CPython implementation"
                    .to_string(),
                license: "Apache-2.0, distributed with the repository".to_string(),
                synthetic: true,
                shape: shape(
                    &[
                        "decision_time",
                        "goal",
                        "oracle",
                        "query_id",
                        "refinement_frontier",
                        "schema_version",
                        "selected_evidence",
                        "selected_factors",
                        "unresolved_obligations",
                        "world_id",
                    ],
                    4_557,
                    &[
                        ("oracle", 3),
                        ("refinement_frontier", 0),
                        ("selected_evidence", 11),
                        ("selected_factors", 6),
                        ("unresolved_obligations", 0),
                    ],
                ),
                notes: "Satisfies 40.33's 'expected outputs are generated independently where \
                        possible': produced by the CPython runtime, not by the crate under test."
                    .to_string(),
            },
            FixtureCard {
                id: GOLDEN_CERTIFICATE_FIXTURE.to_string(),
                path: "fiber-v0.1/golden/reference_certificate.json".to_string(),
                role: FixtureRole::GoldenOutput,
                sha256: "3ea822496a0c98cc6d1ec37633f4076f7a425dccf8f90b739eee5dc4c12de030"
                    .to_string(),
                provenance: "emitted by reference/fiber_runtime/fiber_compile.py".to_string(),
                license: "Apache-2.0, distributed with the repository".to_string(),
                synthetic: true,
                shape: shape(
                    &[
                        "certificate_sha256",
                        "limitations",
                        "omissions",
                        "oracle",
                        "plan",
                        "protected_closure",
                        "query_id",
                        "schema_version",
                        "selected_factors",
                        "selected_facts",
                        "source_hashes",
                        "world_id",
                    ],
                    2_129,
                    &[
                        ("limitations", 1),
                        ("omissions", 4),
                        ("oracle", 3),
                        ("plan", 7),
                        ("protected_closure", 11),
                        ("selected_factors", 6),
                        ("selected_facts", 11),
                        ("source_hashes", 3),
                    ],
                ),
                notes: "The card's own digest covers the whole document including \
                        certificate_sha256; REFERENCE_CERTIFICATE_SHA256 is the digest of the \
                        body without it. The two are different numbers over the same file."
                    .to_string(),
            },
        ],
    )
}

/// The comparator 43.40's release gate demands, as this repository actually has it.
pub fn shipped_baseline() -> BaselineReference {
    BaselineReference {
        name: "graph k-hop, lexical top-k and full-context panel".to_string(),
        method: "bioprism-baseline compares each strategy's selection through the same \
                 deterministic oracle before comparing cost"
            .to_string(),
        evidence: "docs/BASELINE_COMPARISON.md and crates/baseline/tests/equal_engineering.rs"
            .to_string(),
        verdict_preserving: true,
        notes: "On this world the graph walk and the lexical retriever reach the same verdict \
                with the same eleven facts, so the comparison reports no advantage. That is the \
                honest reading and it is why this gate checks that a baseline exists, not that \
                the optimisation won."
            .to_string(),
    }
}

/// The structural conformance profile.
///
/// 40.32 lists "conformance profile" as an input alongside the official fixtures, and this is
/// what that separation buys: the requirements below hold for *any* `fiber-world/0.1` and
/// `fiber-query/0.1` pair a conforming compiler accepts, so they can be re-run against a world
/// generated by `bioprism-worldgen` that no golden fixture describes.
///
/// The point is to find out which requirements are actually about the compiler and which are
/// about the shipped world. Everything omitted here — the eleven protected fact ids, the four
/// witness kinds, the eleven-fact closure size, the published digests — is a statement about the
/// reference fixture, and a suite that never separates the two cannot tell a nonconformant
/// compiler from an unfamiliar world.
///
/// A profile is deliberately *not* releasable on its own: it has no golden layer, because a
/// generated world has no independently produced expected output, and [`crate::assess`] blocks on
/// exactly that. Cases here overlap with [`fiber_suite`] by design — the same requirement stated
/// once in a form that survives the fixture changing.
pub fn structural_profile(
    profile_id: impl Into<String>,
    manifest: FixtureManifest,
    world_fixture: &str,
    query_fixture: &str,
) -> Suite {
    let input = || CaseInput::new(world_fixture, query_fixture);
    let cases = vec![
        CaseBuilder::new(
            "structural.the_section_declares_its_schema_version",
            Layer::Unit,
            "A delivered context names the wire contract it was written against.",
            input(),
        )
        .enforcing(&["43.25", "40.11"])
        .expecting(field(
            artifact::SECTION,
            "/schema_version",
            json!("fiber-decision-section/0.1"),
        ))
        .build(),
        CaseBuilder::new(
            "structural.the_certificate_declares_its_schema_version",
            Layer::Unit,
            "A certificate names its own schema, because its digest is only meaningful relative \
             to a known field set.",
            input(),
        )
        .enforcing(&["43.26", "40.11"])
        .expecting(field(
            artifact::CERTIFICATE,
            "/schema_version",
            json!("fiber-context-certificate/0.1"),
        ))
        .build(),
        CaseBuilder::new(
            "structural.compilation_is_deterministic",
            Layer::Property,
            "Compiling the same pair twice yields byte-identical artifacts, whatever the world.",
            input(),
        )
        .enforcing(&["43.16", "40.32"])
        .expecting(Expectation::RecompilationIsByteIdentical)
        .build(),
        CaseBuilder::new(
            "structural.the_certificate_counts_what_it_lists",
            Layer::Property,
            "The plan's counts and the enumerated selections describe one selection.",
            input(),
        )
        .enforcing(&["43.26", "43.36"])
        .expecting(Expectation::ArrayLenEqualsField {
            artifact: artifact::CERTIFICATE.to_string(),
            pointer: "/selected_facts".to_string(),
            field_pointer: "/plan/compiled_fact_count".to_string(),
        })
        .expecting(Expectation::IntegerIsAtMost {
            artifact: artifact::CERTIFICATE.to_string(),
            pointer: "/plan/compiled_fact_count".to_string(),
            bound: "/plan/total_fact_count".to_string(),
        })
        .build(),
        CaseBuilder::new(
            "structural.the_certificate_digest_verifies",
            Layer::Conformance,
            "A consumer who did not produce the certificate can recompute its digest.",
            input(),
        )
        .enforcing(&["43.26", "40.05"])
        .expecting(Expectation::EmbeddedDigestVerifies {
            artifact: artifact::CERTIFICATE.to_string(),
            digest_field: "certificate_sha256".to_string(),
        })
        .build(),
        CaseBuilder::new(
            "structural.the_certificate_binds_the_delivered_section",
            Layer::Conformance,
            "The section hash on the certificate is the hash of the section shipped with it.",
            input(),
        )
        .enforcing(&["43.26", "40.08"])
        .expecting(Expectation::DigestBinds {
            artifact: artifact::CERTIFICATE.to_string(),
            pointer: "/source_hashes/decision_section_sha256".to_string(),
            target_artifact: artifact::SECTION.to_string(),
        })
        .build(),
        CaseBuilder::new(
            "structural.the_protected_closure_survives_compilation",
            Layer::Conformance,
            "Whatever the world contains, every fact the closure admitted is still in the \
             delivered selection afterwards. Stated without naming any fact id, this is the \
             protected-closure guarantee itself rather than a property of one fixture.",
            input(),
        )
        .enforcing(&["43.13"])
        .expecting(Expectation::ArrayIsSubsetOf {
            artifact: artifact::CERTIFICATE.to_string(),
            subset: "/protected_closure".to_string(),
            superset: "/selected_facts".to_string(),
        })
        .expecting(field(
            artifact::REPORT,
            "/protected_closure_satisfied",
            json!(true),
        ))
        .build(),
        CaseBuilder::new(
            "structural.omissions_are_counted_rather_than_hidden",
            Layer::Conformance,
            "The corpus minus the selection equals the reported omission count.",
            input(),
        )
        .enforcing(&["43.26"])
        .expecting(Expectation::IntegersReconcile {
            artifact: artifact::CERTIFICATE.to_string(),
            minuend: "/plan/total_fact_count".to_string(),
            subtrahend: "/plan/compiled_fact_count".to_string(),
            difference: "/omissions/total_facts".to_string(),
        })
        .expecting(Expectation::ArrayIsNonEmpty {
            artifact: artifact::CERTIFICATE.to_string(),
            pointer: "/limitations".to_string(),
        })
        .build(),
        CaseBuilder::new(
            "structural.the_pair_compiles_to_a_verified_certificate",
            Layer::EndToEnd,
            "Two documents in, a section and a verifiable certificate out.",
            input(),
        )
        .enforcing(&["43.42"])
        .expecting(Expectation::ProducesArtifacts {
            artifacts: vec![
                artifact::SECTION.to_string(),
                artifact::CERTIFICATE.to_string(),
                artifact::CERTIFICATE_EXTENDED.to_string(),
                artifact::REPORT.to_string(),
            ],
        })
        .expecting(Expectation::EmbeddedDigestVerifies {
            artifact: artifact::CERTIFICATE_EXTENDED.to_string(),
            digest_field: "certificate_sha256".to_string(),
        })
        .build(),
    ];

    Suite::new(
        profile_id,
        FIBER_SUITE_VERSION,
        "Structural profile: the FIBER compiler requirements that do not depend on the shipped \
         fixture. Has no golden layer by construction and is not releasable on its own.",
        manifest,
        cases,
    )
}

/// The suite.
pub fn fiber_suite() -> Suite {
    Suite::new(
        FIBER_SUITE_ID,
        FIBER_SUITE_VERSION,
        "Requirements any conforming FIBER query compiler must satisfy against the fiber-v0.1 \
         reference world and leakage query. Conformance is not accuracy (40.32 invariant 1).",
        fiber_fixture_manifest(),
        fiber_cases(),
    )
}

fn fiber_cases() -> Vec<ConformanceCase> {
    let mut cases = Vec::new();
    cases.extend(unit_cases());
    cases.extend(property_cases());
    cases.extend(golden_cases());
    cases.extend(conformance_cases());
    cases.extend(end_to_end_cases());
    cases
}

fn baseline_input() -> CaseInput {
    CaseInput::new(WORLD_FIXTURE, QUERY_FIXTURE)
}

fn unit_cases() -> Vec<ConformanceCase> {
    vec![
        CaseBuilder::new(
            "fiber.unit.section_declares_its_schema_version",
            Layer::Unit,
            "A delivered context names the wire contract it was written against, so a consumer \
             can refuse a version it does not understand instead of misreading it.",
            baseline_input(),
        )
        .enforcing(&["43.25", "40.11"])
        .expecting(field(
            artifact::SECTION,
            "/schema_version",
            json!("fiber-decision-section/0.1"),
        ))
        .build(),
        CaseBuilder::new(
            "fiber.unit.certificate_declares_its_schema_version",
            Layer::Unit,
            "A certificate names its own schema, because its digest is only meaningful relative \
             to a known field set.",
            baseline_input(),
        )
        .enforcing(&["43.26", "40.11"])
        .expecting(field(
            artifact::CERTIFICATE,
            "/schema_version",
            json!("fiber-context-certificate/0.1"),
        ))
        .build(),
        CaseBuilder::new(
            "fiber.unit.the_extended_profile_uses_a_distinct_schema_version",
            Layer::Unit,
            "A profile that adds hashed fields must not reuse the base version, or two documents \
             with the same declared schema will disagree on their digests.",
            baseline_input(),
        )
        .enforcing(&["43.26", "40.37"])
        .expecting(field(
            artifact::CERTIFICATE_EXTENDED,
            "/schema_version",
            json!("fiber-context-certificate/0.2-extended"),
        ))
        .build(),
        CaseBuilder::new(
            "fiber.unit.the_plan_names_the_backend_that_produced_it",
            Layer::Unit,
            "A compiled context states which backend produced it, so a result can be attributed \
             to a strategy rather than to the system in general.",
            baseline_input(),
        )
        .enforcing(&["43.36", "43.37"])
        .expecting(field(
            artifact::CERTIFICATE,
            "/plan/backend",
            json!("backward_factor_slice_reference"),
        ))
        .build(),
        CaseBuilder::new(
            "fiber.unit.the_section_and_certificate_agree_on_identity",
            Layer::Unit,
            "The receipt and the thing it receipts carry the same world and query identity; a \
             certificate for a different query is worse than no certificate.",
            baseline_input(),
        )
        .enforcing(&["40.05", "43.26"])
        .expecting(field(
            artifact::SECTION,
            "/world_id",
            json!("radiogenomic-integrity-demo-v1"),
        ))
        .expecting(field(
            artifact::CERTIFICATE,
            "/world_id",
            json!("radiogenomic-integrity-demo-v1"),
        ))
        .expecting(field(
            artifact::SECTION,
            "/query_id",
            json!("audit-split-integrity-v1"),
        ))
        .expecting(field(
            artifact::CERTIFICATE,
            "/query_id",
            json!("audit-split-integrity-v1"),
        ))
        .build(),
    ]
}

fn property_cases() -> Vec<ConformanceCase> {
    vec![
        CaseBuilder::new(
            "fiber.property.compilation_is_deterministic",
            Layer::Property,
            "Compiling the same world and query twice yields byte-identical artifacts. Without \
             this nothing else in the suite is evidence, because a passing run cannot be \
             distinguished from a lucky one.",
            baseline_input(),
        )
        .enforcing(&["43.16", "40.32", "43.40"])
        .expecting(Expectation::RecompilationIsByteIdentical)
        .build(),
        CaseBuilder::new(
            "fiber.property.the_compiled_region_never_exceeds_the_world",
            Layer::Property,
            "A compiled selection is a subset of the corpus it was drawn from. A plan reporting \
             more compiled facts than the world holds is counting something else.",
            baseline_input(),
        )
        .enforcing(&["43.34", "43.36"])
        .expecting(Expectation::IntegerIsAtMost {
            artifact: artifact::CERTIFICATE.to_string(),
            pointer: "/plan/compiled_fact_count".to_string(),
            bound: "/plan/total_fact_count".to_string(),
        })
        .expecting(Expectation::IntegerIsAtMost {
            artifact: artifact::CERTIFICATE.to_string(),
            pointer: "/plan/compiled_factor_count".to_string(),
            bound: "/plan/total_factor_count".to_string(),
        })
        .build(),
        CaseBuilder::new(
            "fiber.property.the_certificate_counts_what_it_lists",
            Layer::Property,
            "The plan's counts and the enumerated selections describe one selection. A mismatch \
             means the summary a reader trusts and the list an auditor replays have diverged.",
            baseline_input(),
        )
        .enforcing(&["43.26", "43.36"])
        .expecting(Expectation::ArrayLenEqualsField {
            artifact: artifact::CERTIFICATE.to_string(),
            pointer: "/selected_facts".to_string(),
            field_pointer: "/plan/compiled_fact_count".to_string(),
        })
        .expecting(Expectation::ArrayLenEqualsField {
            artifact: artifact::CERTIFICATE.to_string(),
            pointer: "/selected_factors".to_string(),
            field_pointer: "/plan/compiled_factor_count".to_string(),
        })
        .build(),
        CaseBuilder::new(
            "fiber.property.a_budget_equal_to_the_closure_still_compiles",
            Layer::Property,
            "The refusal boundary sits below the protected closure, not at it. A compiler that \
             also refuses a budget exactly equal to the closure is unusable for the case the \
             closure was sized for.",
            baseline_input().overriding_query("/budgets/max_facts", json!(11)),
        )
        .enforcing(&["43.13", "43.25"])
        .expecting(Expectation::ProducesArtifacts {
            artifacts: vec![artifact::CERTIFICATE.to_string()],
        })
        .expecting(Expectation::ArrayLenIs {
            artifact: artifact::CERTIFICATE.to_string(),
            pointer: "/selected_facts".to_string(),
            len: 11,
        })
        .build(),
    ]
}

fn golden_cases() -> Vec<ConformanceCase> {
    vec![
        CaseBuilder::new(
            "fiber.golden.the_decision_section_matches_the_frozen_output",
            Layer::Golden,
            "A conforming compiler emits the same Decision Section bytes as the independently \
             written CPython reference, so a section produced by either replays against the other.",
            baseline_input(),
        )
        .enforcing(&["40.33", "43.25", "43.42"])
        .expecting(Expectation::MatchesGoldenFixture {
            artifact: artifact::SECTION.to_string(),
            fixture: GOLDEN_SECTION_FIXTURE.to_string(),
        })
        .build(),
        CaseBuilder::new(
            "fiber.golden.the_certificate_matches_the_frozen_output",
            Layer::Golden,
            "The Context Certificate is byte-identical to the reference implementation's.",
            baseline_input(),
        )
        .enforcing(&["40.33", "43.26", "43.42"])
        .expecting(Expectation::MatchesGoldenFixture {
            artifact: artifact::CERTIFICATE.to_string(),
            fixture: GOLDEN_CERTIFICATE_FIXTURE.to_string(),
        })
        .build(),
        CaseBuilder::new(
            "fiber.golden.the_published_digests_are_reproduced",
            Layer::Golden,
            "The digests this project publishes are reproducible from a clean checkout. These are \
             the numbers a third party quotes when claiming to have replayed a result.",
            baseline_input(),
        )
        .enforcing(&["40.05", "43.26", "40.33"])
        .expecting(Expectation::EmbeddedDigestIs {
            artifact: artifact::CERTIFICATE.to_string(),
            digest_field: "certificate_sha256".to_string(),
            sha256: REFERENCE_CERTIFICATE_SHA256.to_string(),
        })
        .expecting(Expectation::DocumentDigestIs {
            artifact: artifact::SECTION.to_string(),
            sha256: REFERENCE_SECTION_SHA256.to_string(),
        })
        .expecting(field(
            artifact::CERTIFICATE,
            "/source_hashes/world_sha256",
            json!(REFERENCE_WORLD_SHA256),
        ))
        .expecting(field(
            artifact::CERTIFICATE,
            "/source_hashes/query_sha256",
            json!(REFERENCE_QUERY_SHA256),
        ))
        .build(),
    ]
}

fn conformance_cases() -> Vec<ConformanceCase> {
    vec![
        CaseBuilder::new(
            "fiber.conformance.the_protected_closure_is_retained",
            Layer::Conformance,
            "Every fact carrying a protected tag survives into the delivered context, whether or \
             not a dependency path reaches it. This is the one property the whole design exists \
             to guarantee: a compact context that dropped the evidence which would have changed \
             the decision is worse than no compaction.",
            baseline_input(),
        )
        .enforcing(&["43.13", "43.41"])
        .expecting(Expectation::ArrayContains {
            artifact: artifact::CERTIFICATE.to_string(),
            pointer: "/selected_facts".to_string(),
            values: PROTECTED_FACTS.iter().map(|f| (*f).to_string()).collect(),
        })
        .expecting(Expectation::ArrayIsSubsetOf {
            artifact: artifact::CERTIFICATE.to_string(),
            subset: "/protected_closure".to_string(),
            superset: "/selected_facts".to_string(),
        })
        .expecting(field(
            artifact::REPORT,
            "/protected_closure_satisfied",
            json!(true),
        ))
        .expecting(Expectation::ArrayLenIs {
            artifact: artifact::REPORT.to_string(),
            pointer: "/unmatched_protected_tags".to_string(),
            len: 0,
        })
        .build(),
        CaseBuilder::new(
            "fiber.conformance.the_certificate_digest_verifies",
            Layer::Conformance,
            "A consumer who did not produce the certificate can recompute its digest and get the \
             same answer. A receipt nobody can recompute is an assertion.",
            baseline_input(),
        )
        .enforcing(&["43.26", "40.05"])
        .expecting(Expectation::EmbeddedDigestVerifies {
            artifact: artifact::CERTIFICATE.to_string(),
            digest_field: "certificate_sha256".to_string(),
        })
        .expecting(Expectation::EmbeddedDigestVerifies {
            artifact: artifact::CERTIFICATE_EXTENDED.to_string(),
            digest_field: "certificate_sha256".to_string(),
        })
        .build(),
        CaseBuilder::new(
            "fiber.conformance.the_certificate_binds_the_delivered_section",
            Layer::Conformance,
            "The section hash recorded on the certificate is the hash of the section actually \
             shipped alongside it. A certificate that hashes correctly but describes a different \
             section certifies nothing.",
            baseline_input(),
        )
        .enforcing(&["43.26", "40.08"])
        .expecting(Expectation::DigestBinds {
            artifact: artifact::CERTIFICATE.to_string(),
            pointer: "/source_hashes/decision_section_sha256".to_string(),
            target_artifact: artifact::SECTION.to_string(),
        })
        .build(),
        CaseBuilder::new(
            "fiber.conformance.a_budget_below_the_protected_closure_is_refused",
            Layer::Conformance,
            "When the mandatory evidence does not fit the caller's budget the compiler refuses \
             with a typed error. It does not deliver a smaller context: silently trimming \
             protected evidence to fit is the failure mode the budget was meant to prevent.",
            baseline_input().overriding_query("/budgets/max_facts", json!(4)),
        )
        .enforcing(&["43.13", "43.25", "40.36"])
        .expecting(refusal("budget_exceeded"))
        .build(),
        CaseBuilder::new(
            "fiber.conformance.omissions_are_counted_rather_than_hidden",
            Layer::Conformance,
            "Everything the compiler left out is accounted for: the corpus minus the selection \
             equals the reported omission count, and the reason is stated.",
            baseline_input(),
        )
        .enforcing(&["43.26"])
        .expecting(Expectation::IntegersReconcile {
            artifact: artifact::CERTIFICATE.to_string(),
            minuend: "/plan/total_fact_count".to_string(),
            subtrahend: "/plan/compiled_fact_count".to_string(),
            difference: "/omissions/total_facts".to_string(),
        })
        .expecting(field(
            artifact::CERTIFICATE,
            "/omissions/classification",
            json!("no_backward_dependency_path_or_temporally_inaccessible"),
        ))
        .build(),
        CaseBuilder::new(
            "fiber.conformance.omissions_are_classified_by_influence",
            Layer::Conformance,
            "Omitted evidence is grouped by structural reason and each group carries an influence \
             class from the closed vocabulary, and the groups account for every omitted fact \
             exactly. A count with a classification string cannot distinguish 'provably cannot \
             matter' from 'nobody checked', and only the first supports a sufficiency claim.",
            baseline_input(),
        )
        .enforcing(&["43.26", "43.28"])
        .expecting(Expectation::ArrayPartitionsTotal {
            artifact: artifact::CERTIFICATE_EXTENDED.to_string(),
            pointer: "/omission_manifest/groups".to_string(),
            field: "influence".to_string(),
            allowed: [
                "zero",
                "bounded",
                "inaccessible_by_policy",
                "deferred_acquisition",
                "unknown",
            ]
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
            count_field: "count".to_string(),
            total: "/omissions/total_facts".to_string(),
        })
        .expecting(field(
            artifact::CERTIFICATE_EXTENDED,
            "/supports_sufficiency_claim",
            json!(true),
        ))
        .build(),
        CaseBuilder::new(
            "fiber.conformance.deferred_passes_are_declared_not_skipped",
            Layer::Conformance,
            "A pass the implementation cannot run is reported as deferred with a reason. An \
             engine that omits a pass from its report is indistinguishable from one that ran it \
             and found nothing.",
            baseline_input(),
        )
        .enforcing(&["43.16", "43.06", "43.10", "43.11", "43.12"])
        .expecting(Expectation::ArrayProjectionContains {
            artifact: artifact::REPORT.to_string(),
            pointer: "/deferred_passes".to_string(),
            field: "name".to_string(),
            values: [
                "obstruction_tests",
                "abstract_interpretation",
                "decision_quotient",
                "rate_distortion",
            ]
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
        })
        .expecting(Expectation::ArrayIsNonEmpty {
            artifact: artifact::CERTIFICATE.to_string(),
            pointer: "/limitations".to_string(),
        })
        .build(),
        CaseBuilder::new(
            "fiber.conformance.executed_passes_are_receipted_in_order",
            Layer::Conformance,
            "The pipeline reports the passes it ran, in order. Order is normative: the protected \
             closure is computed before slicing, so that mandatory evidence enters the selection \
             whether or not a dependency path reaches it, and the policy screen runs after it so \
             that a policy withholding a mandatory fact is observable rather than invisible.",
            baseline_input(),
        )
        .enforcing(&["43.16", "43.13", "43.33"])
        .expecting(Expectation::ArrayProjectionEquals {
            artifact: artifact::REPORT.to_string(),
            pointer: "/passes".to_string(),
            field: "name".to_string(),
            values: [
                "protected_closure",
                "backward_slice",
                "policy",
                "temporal_cut",
                "oracle",
                "plan_selection",
            ]
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
        })
        .build(),
        CaseBuilder::new(
            "fiber.conformance.the_oracle_emits_checkable_witnesses",
            Layer::Conformance,
            "The verdict comes with concrete witnesses a human can check, not a score. Each of \
             the four injected leakage mechanisms is named.",
            baseline_input(),
        )
        .enforcing(&["43.41"])
        .expecting(field(artifact::SECTION, "/oracle/status", json!("invalid")))
        .expecting(Expectation::ArrayProjectionContains {
            artifact: artifact::SECTION.to_string(),
            pointer: "/oracle/witnesses".to_string(),
            field: "type".to_string(),
            values: [
                "identity_leakage",
                "site_leakage",
                "temporal_leakage",
                "preprocessing_leakage",
            ]
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
        })
        .build(),
        CaseBuilder::new(
            "fiber.conformance.exploratory_evidence_is_not_admitted",
            Layer::Conformance,
            "No fact that only an expansion heuristic would reach enters the selection. The \
             reference world attaches 750 exploratory factors to a hub the selection touches, so \
             a compiler that widens by one undirected hop admits the entire corpus.",
            baseline_input(),
        )
        .enforcing(&["43.17", "43.34"])
        .expecting(Expectation::NoArrayMemberStartsWith {
            artifact: artifact::CERTIFICATE.to_string(),
            pointer: "/selected_facts".to_string(),
            prefix: "fact.explore.".to_string(),
        })
        .expecting(Expectation::NoArrayMemberStartsWith {
            artifact: artifact::CERTIFICATE.to_string(),
            pointer: "/selected_factors".to_string(),
            prefix: "factor.explore.".to_string(),
        })
        .build(),
        CaseBuilder::new(
            "fiber.conformance.a_query_on_an_unsupported_schema_is_refused",
            Layer::Conformance,
            "A query declaring a schema the compiler does not implement is refused by version, \
             not parsed opportunistically. Accepting it would let a v0.9 query be interpreted \
             under v0.1 semantics with no signal to anyone.",
            baseline_input().overriding_query("/schema_version", json!("fiber-query/0.9")),
        )
        .enforcing(&["40.11", "40.36", "40.37"])
        .expecting(refusal("unsupported_query_schema"))
        .build(),
        CaseBuilder::new(
            "fiber.conformance.a_world_on_an_unsupported_schema_is_refused",
            Layer::Conformance,
            "The same version discipline applies to the world. Both implementations must reject \
             the same documents, or 'compiles successfully' means different things in each.",
            baseline_input().overriding_world("/schema_version", json!("fiber-world/0.9")),
        )
        .enforcing(&["40.11", "40.36", "40.37"])
        .expecting(refusal("unsupported_world_schema"))
        .build(),
        CaseBuilder::new(
            "fiber.conformance.an_empty_query_identifier_is_refused",
            Layer::Conformance,
            "Identifiers are validated before use rather than carried into a certificate. An \
             empty id in a receipt makes the receipt unindexable and the result unattributable, \
             and 40.05 forbids conflating identities that an empty string would silently merge.",
            baseline_input().overriding_query("/query_id", json!("")),
        )
        .enforcing(&["40.05", "40.36"])
        .expecting(refusal("invalid_identifier"))
        .build(),
        CaseBuilder::new(
            "fiber.conformance.an_undeclared_query_field_is_refused",
            Layer::Conformance,
            "A query carrying a key the format does not declare is refused, and the refusal names \
             the key. The document is hashed whole into the certificate, so a key the compiler \
             ignores semantically it still honours cryptographically: reading past it would let \
             the same compiled answer be issued under two different identities. Both \
             implementations must reject the same documents, or a caller cannot tell which of \
             them is wrong.",
            baseline_input().inserting_into_query("/decision_loss", json!({"models": ["m1"]})),
        )
        .enforcing(&["40.11", "40.36", "40.37", "40.05"])
        .expecting(refusal_naming("undeclared_query_field", &["decision_loss"]))
        .build(),
        CaseBuilder::new(
            "fiber.conformance.an_undeclared_field_inside_budgets_is_refused",
            Layer::Conformance,
            "The declared set is closed inside the format's one nested object too, and the \
             refusal names the key by the path it was found at rather than by its bare name. A \
             compiler that screens only the top level accepts a misspelled budget and then \
             compiles against a limit the caller never set, which is the failure that screening \
             undeclared keys exists to prevent.",
            baseline_input().inserting_into_query("/budgets/max_items", json!(3)),
        )
        .enforcing(&["40.11", "40.36", "40.37"])
        .expecting(refusal_naming(
            "undeclared_query_field",
            &["budgets.max_items"],
        ))
        .build(),
    ]
}

fn end_to_end_cases() -> Vec<ConformanceCase> {
    vec![
        CaseBuilder::new(
            "fiber.end_to_end.a_world_and_query_compile_to_a_verified_certificate",
            Layer::EndToEnd,
            "The whole vertical slice: two documents in, a Decision Section and a Context \
             Certificate out, the certificate's digest recomputing and binding the section, and \
             the oracle reaching the verdict the world was built to provoke.",
            baseline_input(),
        )
        .enforcing(&["43.42", "40.44"])
        .expecting(Expectation::ProducesArtifacts {
            artifacts: vec![
                artifact::SECTION.to_string(),
                artifact::CERTIFICATE.to_string(),
                artifact::CERTIFICATE_EXTENDED.to_string(),
                artifact::REPORT.to_string(),
            ],
        })
        .expecting(Expectation::EmbeddedDigestVerifies {
            artifact: artifact::CERTIFICATE.to_string(),
            digest_field: "certificate_sha256".to_string(),
        })
        .expecting(Expectation::DigestBinds {
            artifact: artifact::CERTIFICATE.to_string(),
            pointer: "/source_hashes/decision_section_sha256".to_string(),
            target_artifact: artifact::SECTION.to_string(),
        })
        .expecting(field(
            artifact::CERTIFICATE,
            "/oracle/status",
            json!("invalid"),
        ))
        .build(),
        CaseBuilder::new(
            "fiber.end_to_end.editing_the_world_moves_the_recorded_world_hash",
            Layer::EndToEnd,
            "A byte changed anywhere in the world propagates to the world hash on the \
             certificate, while a change outside the compiled region leaves the section \
             untouched. Together these say the certificate binds its inputs and the section \
             depends only on what was compiled.",
            baseline_input().overriding_world(
                "/description",
                json!("conformance probe: an edit outside the compiled region"),
            ),
        )
        .enforcing(&["43.26", "40.05", "43.34"])
        .expecting(Expectation::FieldDiffersFrom {
            artifact: artifact::CERTIFICATE.to_string(),
            pointer: "/source_hashes/world_sha256".to_string(),
            value: json!(REFERENCE_WORLD_SHA256),
        })
        .expecting(Expectation::DocumentDigestIs {
            artifact: artifact::SECTION.to_string(),
            sha256: REFERENCE_SECTION_SHA256.to_string(),
        })
        .expecting(Expectation::EmbeddedDigestVerifies {
            artifact: artifact::CERTIFICATE.to_string(),
            digest_field: "certificate_sha256".to_string(),
        })
        .build(),
    ]
}

/// The Rust engine, as one implementation under test.
#[derive(Debug, Clone, Copy, Default)]
pub struct FiberReference;

impl Implementation for FiberReference {
    fn identity(&self) -> ImplementationIdentity {
        // No digest: an in-process library cannot hash its own compiled artifact, so this
        // certificate binds a name and version only. See `ImplementationIdentity::is_digest_bound`.
        ImplementationIdentity::new("bioprism-fiber", env!("CARGO_PKG_VERSION"), "rust")
    }

    fn compile(&self, world: &Value, query: &Value) -> Result<CompileArtifacts, CompileFailure> {
        let world = World::from_json(world.clone()).map_err(world_failure)?;
        let query = Query::from_json(query.clone()).map_err(fiber_failure)?;
        let output = compile(&world, &query).map_err(fiber_failure)?;

        let certificate = output
            .certificate
            .to_json(CertificateProfile::Reference)
            .map_err(|e| CompileFailure::new("canonical_encoding", e.to_string()))?;
        let extended = output
            .certificate
            .to_json(CertificateProfile::Extended)
            .map_err(|e| CompileFailure::new("canonical_encoding", e.to_string()))?;

        Ok(CompileArtifacts::new()
            .with(artifact::SECTION, output.section.to_json())
            .with(artifact::CERTIFICATE, certificate)
            .with(artifact::CERTIFICATE_EXTENDED, extended)
            .with(artifact::REPORT, compiler_report(&output)))
    }
}

/// The compiler's self-report, projected onto the wire.
///
/// [`bioprism_fiber::CompileTrace`] is a Rust value; a conformance case may only read documents.
/// Publishing the trace as JSON is what makes "declares the passes it could not run" a
/// certifiable requirement rather than an internal invariant.
fn compiler_report(output: &CompileOutput) -> Value {
    json!({
        "schema_version": FIBER_REPORT_SCHEMA_VERSION,
        "passes": output
            .trace
            .passes
            .iter()
            .map(|pass| json!({
                "name": pass.name,
                "retained": pass.retained,
                "note": pass.note,
            }))
            .collect::<Vec<Value>>(),
        "deferred_passes": output
            .trace
            .deferred_passes
            .iter()
            .map(|(name, reason)| json!({ "name": name, "reason": reason }))
            .collect::<Vec<Value>>(),
        "protected_closure_satisfied": output.protected_closure_satisfied(),
        "unmatched_protected_tags": output.trace.unmatched_protected_tags,
        "dropped_protected": output.trace.dropped_protected,
        "withheld_variables": output
            .trace
            .temporal_cut
            .withheld()
            .cloned()
            .collect::<Vec<String>>(),
    })
}

/// Maps [`FiberError`] onto the stable failure kinds the suite asserts on.
///
/// The mapping is the contract, not the crate's variant names: an implementation in another
/// language reproduces these strings, not this enum.
fn fiber_failure(error: FiberError) -> CompileFailure {
    let kind = match &error {
        FiberError::UnsupportedQuerySchema { .. } => "unsupported_query_schema",
        FiberError::QueryNotAnObject => "query_not_an_object",
        FiberError::UnknownQueryFields { .. } => "undeclared_query_field",
        FiberError::MissingQueryField(_) => "missing_query_field",
        FiberError::WrongQueryFieldType { .. } => "wrong_query_field_type",
        FiberError::InvalidIdentifier(_) => "invalid_identifier",
        FiberError::InvalidDecisionTime(_) => "invalid_decision_time",
        FiberError::BudgetExceeded { .. } => "budget_exceeded",
        FiberError::UnorderableSplitGroups { .. } => "unorderable_split_groups",
        FiberError::World(inner) => return world_failure(inner.clone()),
        FiberError::Policy(inner) => return policy_failure(inner.clone()),
    };
    CompileFailure::new(kind, error.to_string())
}

/// Names a policy refusal (43.33, 40.25).
///
/// Kept separate from [`fiber_failure`] for the same reason [`world_failure`] is: the four kinds
/// are decisions the compiler already distinguishes, and collapsing them here would tell a
/// conformance consumer less than the compiler knows. `protected_closure_withheld_by_policy` in
/// particular is not interchangeable with the other three — it is 43.13's mandatory closure
/// failing, which no grant obtained later can turn into a partial answer.
fn policy_failure(error: PolicyViolation) -> CompileFailure {
    let kind = match &error {
        PolicyViolation::Conflict { .. } => "policy_conflict",
        PolicyViolation::MalformedDataPolicy { .. } => "malformed_data_policy",
        PolicyViolation::UninterpretableRequirement { .. } => "uninterpretable_policy_requirement",
        PolicyViolation::ProtectedClosureWithheld { .. } => "protected_closure_withheld_by_policy",
    };
    CompileFailure::new(kind, error.to_string())
}

fn world_failure(error: WorldError) -> CompileFailure {
    let kind = match &error {
        WorldError::UnsupportedSchema { .. } => "unsupported_world_schema",
        WorldError::DuplicateFactId(_) => "duplicate_fact_id",
        WorldError::DuplicateFactorId(_) => "duplicate_factor_id",
        WorldError::UnknownFactorInputs { .. } => "unknown_factor_inputs",
        WorldError::NotAnObject => "world_not_an_object",
        WorldError::MissingField { .. } => "missing_world_field",
        WorldError::WrongType { .. } => "wrong_world_field_type",
        WorldError::Scope { .. } => "invalid_scope",
        WorldError::Timestamp { .. } => "invalid_timestamp",
        WorldError::Identifier { .. } => "invalid_world_identifier",
    };
    CompileFailure::new(kind, error.to_string())
}

fn field(artifact: &str, pointer: &str, value: Value) -> Expectation {
    Expectation::FieldEquals {
        artifact: artifact.to_string(),
        pointer: pointer.to_string(),
        value,
    }
}

/// A refusal asserted by kind alone.
fn refusal(error_kind: &str) -> Expectation {
    Expectation::FailsWith {
        error_kind: error_kind.to_string(),
        naming: Vec::new(),
    }
}

/// A refusal that must also quote the part of the input it rejected.
///
/// `fragments` are always drawn from the case's own overrides, never from this crate's wording;
/// see [`Expectation::FailsWith`] for why that is the only form the assertion may take.
fn refusal_naming(error_kind: &str, fragments: &[&str]) -> Expectation {
    Expectation::FailsWith {
        error_kind: error_kind.to_string(),
        naming: fragments.iter().map(|f| (*f).to_string()).collect(),
    }
}

fn shape(keys: &[&str], canonical_bytes: usize, sizes: &[(&str, usize)]) -> FixtureShape {
    FixtureShape {
        top_level_keys: keys.iter().map(|k| (*k).to_string()).collect(),
        canonical_bytes,
        collection_sizes: sizes
            .iter()
            .map(|(key, size)| ((*key).to_string(), *size))
            .collect::<BTreeMap<String, usize>>(),
    }
}
