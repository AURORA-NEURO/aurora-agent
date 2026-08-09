//! Conformance cases: the unit of certification.
//!
//! Blueprint 40.32 wants a third party to run the official suite against their own
//! implementation and receive a per-requirement verdict. That is only possible if the cases are
//! **data**. A case written as a Rust `#[test]` certifies the Rust engine and nothing else; a
//! case written as a document — input fixture, declarative expectation, the invariant it
//! establishes, the blueprint module it enforces — can be shipped, versioned, and executed by a
//! CPython or TypeScript runner that never links this crate.
//!
//! So every predicate a case may assert is drawn from the closed vocabulary in [`Expectation`].
//! The vocabulary is deliberately generic — pointers, arrays, digests — rather than a set of
//! FIBER-shaped verbs, because a suite whose expectations name domain concepts cannot be reused
//! for the next contract and quietly becomes code again.
//!
//! What this does **not** give you is arbitrary predicates. A property that cannot be phrased in
//! the vocabulary must either extend the vocabulary (and so extend the suite schema version) or
//! live as an ordinary unit test in the implementation's own repository. That is a real limit,
//! and it is the price of the cases being portable.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

/// Where a case sits in the test pyramid of 40.31.
///
/// Ordered cheap-to-expensive, and that ordering is load-bearing: [`crate::pyramid`] uses it to
/// detect an inverted pyramid, where the slow end-to-end layer outnumbers the fast layers that
/// are supposed to catch the same defects first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    /// One field of one wire contract. Fast, and diagnostic when it fails.
    Unit,
    /// A statement quantified over inputs — determinism, monotonicity, self-consistency —
    /// checked here on the shipped fixtures but written as a universal.
    Property,
    /// Byte equality against a frozen expected output (40.33).
    Golden,
    /// A published requirement any conforming implementation must satisfy (40.32).
    Conformance,
    /// The whole vertical slice: world in, verified certificate out.
    EndToEnd,
}

impl Layer {
    pub const ALL: [Layer; 5] = [
        Layer::Unit,
        Layer::Property,
        Layer::Golden,
        Layer::Conformance,
        Layer::EndToEnd,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Layer::Unit => "unit",
            Layer::Property => "property",
            Layer::Golden => "golden",
            Layer::Conformance => "conformance",
            Layer::EndToEnd => "end_to_end",
        }
    }
}

impl fmt::Display for Layer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Whether failing this case blocks certification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Requirement {
    /// A conforming implementation must pass. Failure blocks the certificate.
    Must,
    /// Recommended. Failure is disclosed in the bundle but does not block.
    Should,
}

impl Requirement {
    pub fn as_str(self) -> &'static str {
        match self {
            Requirement::Must => "must",
            Requirement::Should => "should",
        }
    }
}

/// Whether an override rewrites a location the fixture has, or adds one it does not.
///
/// The two are separate operations rather than one "write this pointer" because the mistake they
/// have to catch is opposite in each direction. A [`Replace`](OverrideOp::Replace) whose pointer is
/// absent is a typo in the case, and silently creating the location would let the case test a
/// document the fixture never described — 40.33's "fixture accidentally trivial" arriving by a
/// different door, which is what [`crate::ConformanceError::PointerNotFound`] exists to stop. An
/// [`Insert`](OverrideOp::Insert) whose key is *present* is that same defect mirrored: the case
/// says the baseline lacks the key, the baseline has grown it, and the variant is now identical to
/// the document it was supposed to differ from.
///
/// So neither operation is a superset of the other, and a case picks the one that states what it
/// means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverrideOp {
    /// Rewrite the value at an existing location. Absent location is an error.
    #[default]
    Replace,
    /// Add a key to an existing object. Absent parent, non-object parent, or a key that is
    /// already there is an error.
    Insert,
}

impl OverrideOp {
    pub fn as_str(self) -> &'static str {
        match self {
            OverrideOp::Replace => "replace",
            OverrideOp::Insert => "insert",
        }
    }

    fn is_replace(&self) -> bool {
        matches!(self, OverrideOp::Replace)
    }
}

/// A single edit applied to a fixture before compilation.
///
/// This is how a case says "the reference query, but with `max_facts` set to 4" without shipping
/// a near-duplicate 405-byte file for every negative variant. 40.33 asks for malformed, stale and
/// adversarial variants of each fixture; expressing them as a pointer and a value keeps the
/// variant's *difference from the baseline* visible in the case, which is the part a reviewer
/// needs to read.
///
/// # What an override can and cannot express
///
/// With [`OverrideOp::Replace`] and [`OverrideOp::Insert`] a case can state any variant that
/// differs from the baseline by a changed value or an added key, at any depth — including the
/// undeclared key a `fiber-query/0.2` compiler must refuse, which a pointer alone cannot reach
/// because a JSON pointer addresses locations that exist.
///
/// It cannot remove anything, cannot add a member to an array, and cannot create the intermediate
/// objects along a path the fixture does not have. A variant that *omits* a required field — the
/// input a compiler must refuse with `missing_query_field` — is therefore still not expressible
/// here, and neither is a world holding one fact more or fewer. Those need a fixture, and a
/// fixture needs a card (40.33).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Override {
    pub pointer: String,
    pub value: Value,
    /// Defaulted and omitted when it is [`OverrideOp::Replace`], so that a third-party runner
    /// written against the original two-field shape still reads every case that does not insert.
    #[serde(default, skip_serializing_if = "OverrideOp::is_replace")]
    pub op: OverrideOp,
}

impl Override {
    pub fn new(pointer: impl Into<String>, value: Value) -> Self {
        Override {
            pointer: pointer.into(),
            value,
            op: OverrideOp::Replace,
        }
    }

    pub fn inserting(pointer: impl Into<String>, value: Value) -> Self {
        Override {
            pointer: pointer.into(),
            value,
            op: OverrideOp::Insert,
        }
    }
}

/// The inputs a case feeds to the implementation under test.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaseInput {
    /// Fixture id of the world document.
    pub world: String,
    /// Fixture id of the query document.
    pub query: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub world_overrides: Vec<Override>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub query_overrides: Vec<Override>,
}

impl CaseInput {
    pub fn new(world: impl Into<String>, query: impl Into<String>) -> Self {
        CaseInput {
            world: world.into(),
            query: query.into(),
            world_overrides: Vec::new(),
            query_overrides: Vec::new(),
        }
    }

    pub fn overriding_query(mut self, pointer: impl Into<String>, value: Value) -> Self {
        self.query_overrides.push(Override::new(pointer, value));
        self
    }

    pub fn overriding_world(mut self, pointer: impl Into<String>, value: Value) -> Self {
        self.world_overrides.push(Override::new(pointer, value));
        self
    }

    pub fn inserting_into_query(mut self, pointer: impl Into<String>, value: Value) -> Self {
        self.query_overrides
            .push(Override::inserting(pointer, value));
        self
    }

    pub fn inserting_into_world(mut self, pointer: impl Into<String>, value: Value) -> Self {
        self.world_overrides
            .push(Override::inserting(pointer, value));
        self
    }
}

/// The closed vocabulary of checkable predicates.
///
/// Each variant reads a named artifact the implementation published and asserts something about
/// it. Nothing here inspects implementation internals, which is what makes the same case runnable
/// against a compiler written in another language.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Expectation {
    /// Compilation succeeds and publishes every named artifact.
    ProducesArtifacts { artifacts: Vec<String> },

    /// Compilation refuses, with this typed failure kind, quoting every fragment in `naming`.
    ///
    /// The point is the *kind*, not the message. 40.31 requires deterministic negative tests on
    /// high-risk paths, and a negative test that accepts any error would pass against an
    /// implementation that crashed for an unrelated reason.
    ///
    /// `naming` is the only thing a case may say about the message, and it is deliberately narrow:
    /// every fragment must be a piece of the case's *own input* — a key it inserted, an identifier
    /// it set — never English prose. That keeps the requirement satisfiable in any language, since
    /// what has to appear is the caller's own text rather than this crate's wording, while still
    /// separating a compiler that says which key it rejected from one that says only "invalid
    /// query" and sends the caller back through the whole document.
    ///
    /// It is a floor rather than evidence of a good message: an implementation that echoes its
    /// entire input into every refusal satisfies it without telling anyone anything.
    FailsWith {
        error_kind: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        naming: Vec<String>,
    },

    FieldEquals {
        artifact: String,
        pointer: String,
        value: Value,
    },

    /// The value at `pointer` is anything other than `value`.
    ///
    /// Used for the binding properties: change a byte of the world and the recorded world hash
    /// must move.
    FieldDiffersFrom {
        artifact: String,
        pointer: String,
        value: Value,
    },

    ArrayContains {
        artifact: String,
        pointer: String,
        values: Vec<String>,
    },

    ArrayLenIs {
        artifact: String,
        pointer: String,
        len: usize,
    },

    ArrayIsNonEmpty { artifact: String, pointer: String },

    /// Every member of the array at `subset` also appears in the array at `superset`.
    ArrayIsSubsetOf {
        artifact: String,
        subset: String,
        superset: String,
    },

    NoArrayMemberStartsWith {
        artifact: String,
        pointer: String,
        prefix: String,
    },

    /// Projecting `field` out of each object in the array at `pointer` yields a set containing
    /// every name in `values`.
    ArrayProjectionContains {
        artifact: String,
        pointer: String,
        field: String,
        values: Vec<String>,
    },

    /// As [`Expectation::ArrayProjectionContains`], but the projection must equal `values`
    /// exactly, in order. Use where the order is itself the contract.
    ArrayProjectionEquals {
        artifact: String,
        pointer: String,
        field: String,
        values: Vec<String>,
    },

    /// The array at `pointer` partitions the integer at `total`.
    ///
    /// Each object carries a `field` drawn from `allowed`, and the `count_field` values sum to
    /// `total`. This is how the omission manifest of 43.26 is checked without the runner knowing
    /// what an influence class is: whatever the classes are, they must account for every omitted
    /// item exactly once.
    ArrayPartitionsTotal {
        artifact: String,
        pointer: String,
        field: String,
        allowed: Vec<String>,
        count_field: String,
        total: String,
    },

    /// The array at `pointer` has as many members as the integer at `field_pointer`.
    ArrayLenEqualsField {
        artifact: String,
        pointer: String,
        field_pointer: String,
    },

    /// `minuend - subtrahend == difference`, all read as integers from the same artifact.
    IntegersReconcile {
        artifact: String,
        minuend: String,
        subtrahend: String,
        difference: String,
    },

    IntegerIsAtMost {
        artifact: String,
        pointer: String,
        bound: String,
    },

    /// Recomputing the canonical digest of the artifact with `digest_field` removed reproduces
    /// the value stored there.
    ///
    /// This is the check a consumer performs before trusting a certificate it did not produce
    /// (43.26), so a conforming implementation must make it possible: an artifact whose digest
    /// cannot be independently recomputed is not a receipt, it is an assertion.
    EmbeddedDigestVerifies {
        artifact: String,
        digest_field: String,
    },

    /// The digest stored at `digest_field` equals a published constant.
    EmbeddedDigestIs {
        artifact: String,
        digest_field: String,
        sha256: String,
    },

    /// The canonical digest of the whole artifact equals a published constant.
    DocumentDigestIs { artifact: String, sha256: String },

    /// The digest recorded at `pointer` equals the canonical digest of `target_artifact` as
    /// actually delivered.
    ///
    /// Distinct from [`Expectation::EmbeddedDigestIs`]: this catches a certificate that hashes
    /// correctly but describes a *different* section than the one shipped alongside it.
    DigestBinds {
        artifact: String,
        pointer: String,
        target_artifact: String,
    },

    /// The artifact is canonically identical to a frozen golden fixture.
    MatchesGoldenFixture { artifact: String, fixture: String },

    /// Compiling the same inputs a second time yields byte-identical artifacts.
    ///
    /// 40.32 lists "nondeterministic result" as a failure class in its own right, before any
    /// question of correctness: a suite cannot certify what it cannot reproduce.
    RecompilationIsByteIdentical,
}

impl Expectation {
    /// The discriminant name, used by the release gates of 40.44 to ask what a run actually
    /// exercised rather than trusting that a green suite covered the right things.
    pub fn kind(&self) -> &'static str {
        match self {
            Expectation::ProducesArtifacts { .. } => "produces_artifacts",
            Expectation::FailsWith { .. } => "fails_with",
            Expectation::FieldEquals { .. } => "field_equals",
            Expectation::FieldDiffersFrom { .. } => "field_differs_from",
            Expectation::ArrayContains { .. } => "array_contains",
            Expectation::ArrayLenIs { .. } => "array_len_is",
            Expectation::ArrayIsNonEmpty { .. } => "array_is_non_empty",
            Expectation::ArrayIsSubsetOf { .. } => "array_is_subset_of",
            Expectation::NoArrayMemberStartsWith { .. } => "no_array_member_starts_with",
            Expectation::ArrayProjectionContains { .. } => "array_projection_contains",
            Expectation::ArrayProjectionEquals { .. } => "array_projection_equals",
            Expectation::ArrayPartitionsTotal { .. } => "array_partitions_total",
            Expectation::ArrayLenEqualsField { .. } => "array_len_equals_field",
            Expectation::IntegersReconcile { .. } => "integers_reconcile",
            Expectation::IntegerIsAtMost { .. } => "integer_is_at_most",
            Expectation::EmbeddedDigestVerifies { .. } => "embedded_digest_verifies",
            Expectation::EmbeddedDigestIs { .. } => "embedded_digest_is",
            Expectation::DocumentDigestIs { .. } => "document_digest_is",
            Expectation::DigestBinds { .. } => "digest_binds",
            Expectation::MatchesGoldenFixture { .. } => "matches_golden_fixture",
            Expectation::RecompilationIsByteIdentical => "recompilation_is_byte_identical",
        }
    }

    /// Whether this expectation asserts that compilation refuses.
    pub fn expects_refusal(&self) -> bool {
        matches!(self, Expectation::FailsWith { .. })
    }
}

/// One certifiable requirement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConformanceCase {
    pub id: String,
    pub title: String,
    /// The invariant this case establishes, stated as a claim about any conforming
    /// implementation — not as a description of what the code does.
    pub invariant: String,
    /// Blueprint module ids this case enforces, e.g. `43.13`. A case that enforces nothing is a
    /// regression test, not a conformance requirement, and belongs in the implementation's own
    /// test suite.
    pub enforces: Vec<String>,
    pub layer: Layer,
    pub requirement: Requirement,
    pub input: CaseInput,
    pub expect: Vec<Expectation>,
}

impl ConformanceCase {
    pub fn expectation_kinds(&self) -> Vec<String> {
        self.expect.iter().map(|e| e.kind().to_string()).collect()
    }
}

/// Builder for the suite constructors. Keeps the case tables readable enough to review as
/// specification rather than as code.
pub struct CaseBuilder {
    case: ConformanceCase,
}

impl CaseBuilder {
    pub fn new(
        id: impl Into<String>,
        layer: Layer,
        invariant: impl Into<String>,
        input: CaseInput,
    ) -> Self {
        let id = id.into();
        CaseBuilder {
            case: ConformanceCase {
                title: id.clone(),
                id,
                invariant: invariant.into(),
                enforces: Vec::new(),
                layer,
                requirement: Requirement::Must,
                input,
                expect: Vec::new(),
            },
        }
    }

    pub fn titled(mut self, title: impl Into<String>) -> Self {
        self.case.title = title.into();
        self
    }

    pub fn enforcing(mut self, modules: &[&str]) -> Self {
        self.case.enforces = modules.iter().map(|m| (*m).to_string()).collect();
        self
    }

    pub fn recommended(mut self) -> Self {
        self.case.requirement = Requirement::Should;
        self
    }

    pub fn expecting(mut self, expectation: Expectation) -> Self {
        self.case.expect.push(expectation);
        self
    }

    pub fn build(self) -> ConformanceCase {
        self.case
    }
}
