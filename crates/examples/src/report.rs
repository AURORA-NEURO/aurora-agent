//! What a slice actually produced.
//!
//! Blueprint 19.06 (result bundle) and 43.26 (context certificate) both insist that a run is
//! judged from a recorded artefact rather than from console output. These types are that artefact
//! for a vertical slice: everything a reader would need in order to disagree with the slice's
//! verdict, in a form that serialises to canonical JSON and hashes.
//!
//! Two things are recorded that a passing-only report would drop. Every compile carries its
//! `deferred_passes` — the passes `bioprism-fiber` declined to run and why — so a green slice
//! never reads as a complete pipeline. And every observation is kept even when the expectation
//! did not mention it, so a later contributor tightening an expectation can see what the value
//! already was instead of guessing.

use crate::property::Property;
use bioprism_ids::{CanonicalError, ContentHash};
use bioprism_section::{LeakageWitness, OracleStatus, UnresolvedObligation};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One pass receipt from the compiler pipeline (43.16).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PassObservation {
    pub name: String,
    pub retained: usize,
    pub note: String,
}

/// A pass the compiler declined to run, with the reason it gave.
///
/// The reason is carried, not just the name. "abstract_interpretation was skipped" invites the
/// reader to assume it is coming; "fiber-world/0.1 carries no abstract-domain registry" tells
/// them what would have to change first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeferredPass {
    pub pass: String,
    pub reason: String,
}

/// Everything the compiler produced on a slice that compiled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompiledObservation {
    pub status: OracleStatus,
    pub witness_kinds: Vec<String>,
    pub witnesses: Vec<LeakageWitness>,
    pub selected_facts: Vec<String>,
    pub selected_factors: Vec<String>,
    pub protected_closure: Vec<String>,
    /// False when the temporal cut removed protected evidence, which 43.13 treats as blocking.
    pub protected_closure_satisfied: bool,
    pub dropped_protected: Vec<String>,
    pub unmatched_protected_tags: Vec<String>,
    pub unresolved_obligations: Vec<UnresolvedObligation>,
    pub refinement_frontier_actions: Vec<String>,
    pub omission_influence_classes: Vec<String>,
    /// Facts the selection asked for and the temporal cut removed, by id.
    ///
    /// Recorded separately from `dropped_protected` because the two are different failures and
    /// 43.09 treats them differently: a withheld *protected* fact breaks the mandatory closure,
    /// and a withheld unprotected one leaves the closure intact while removing evidence the
    /// decision depended on. A report carrying only the first cannot tell the second from a world
    /// that never had the evidence.
    pub inaccessible_selected_before_cut: Vec<String>,
    pub omitted_fact_count: usize,
    pub supports_sufficiency_claim: bool,
    /// Passes the compiler declined to run, each with the reason. Never empty in v0.1.
    pub deferred_passes: Vec<DeferredPass>,
    pub passes: Vec<PassObservation>,
    pub backend: String,
    pub compiled_factor_count: usize,
    pub section_digest: String,
    pub certificate_digest_reference: String,
    pub certificate_digest_extended: String,
    /// Whether both certificate profiles recompute their own embedded digest.
    pub certificate_verifies: bool,
}

/// The typed reason a compile refused.
///
/// Every `bioprism_fiber::FiberError` variant maps onto exactly one code, and the mapping is
/// written without a wildcard arm. A new failure mode in the compiler therefore breaks this crate
/// at compile time and forces a decision, rather than arriving as an unnamed catch-all that every
/// refusal expectation would silently accept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefusalCode {
    /// Protected closure plus dependency slice exceed `budgets.max_facts` (43.13, 43.25).
    BudgetExceeded,
    /// Subjects sharing an alias have a missing split assignment alongside present ones.
    UnorderableSplitGroups,
    /// The query document is not a well-formed `fiber-query/0.1` contract.
    MalformedQuery,
    /// The world document is not a well-formed `fiber-world/0.1` document.
    MalformedWorld,
    /// Policy refused the compile: a clause the corpus never granted, a malformed or
    /// uninterpretable policy declaration, or a protected fact the caller cannot unlock
    /// (43.33, 40.25).
    ///
    /// This is one code rather than four because the taxonomy lives in
    /// `bioprism_fiber::PolicyViolation`. Splitting it here would duplicate a distinction the
    /// compiler already draws.
    PolicyRefused,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RefusalObservation {
    pub code: RefusalCode,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_facts: Option<usize>,
}

/// One depth of the neighbourhood-walk probe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DepthObservation {
    pub depth: usize,
    pub facts_selected: usize,
    /// Whether the same deterministic oracle, fed this selection, reproduces the full-context
    /// verdict with the same witnesses.
    pub verdict_preserving: bool,
    /// Fraction of the query's protected facts the walk retained.
    pub protected_recall: f64,
    /// Sound, closed, and smaller than half the world: the three conditions together.
    pub usable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphWalkObservation {
    pub max_depth: usize,
    pub depths: Vec<DepthObservation>,
    pub usable_depths: Vec<usize>,
}

/// What became of the result bundle built from a slice's own compile (34.14, 19.06).
///
/// Every field a reader would need in order to disagree with the bundle's verdict, including the
/// three that limit it. `not_recomputed` names the entries that travelled as a digest and were
/// therefore not checked at all; `without_the_key` records what a reviewer who does not hold the
/// producing secret learns, which is nothing; and `verifier_forgery_is_identical` records that a
/// reviewer who does hold it can mint the same bytes. A bundle observation carrying only the
/// successful verification would read as third-party verifiability, which this workspace cannot do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleObservation {
    pub bundle_id: String,
    pub manifest_digest: String,
    /// Entries whose carried content was rehashed and matched, in manifest order.
    pub recomputed_entries: Vec<String>,
    /// Entries recorded by digest only. Never a pass.
    pub not_recomputed: Vec<String>,
    /// Whether the carried Context Certificate satisfied 43.26's own self-verification.
    pub embedded_certificate: String,
    /// Whether the bundle verified after being serialised and parsed back, as a consumer would.
    pub survives_json_round_trip: bool,
    /// The key whose holder produced the tag. Never a party; see `bioprism_bundle::attestation`.
    pub authenticated_key: String,
    /// The tag, in its own wire form, which names its algorithm and cannot be quoted as a signature.
    pub tag: String,
    pub scheme: String,
    pub repudiability: String,
    /// The outcome of offering the attestation to a reviewer holding a different key.
    pub without_the_key: String,
    /// Whether a second holder of the producing secret mints a byte-identical bundle.
    pub verifier_forgery_is_identical: bool,
    /// The sentence a surface displaying this bundle must print.
    pub honest_label: String,
}

/// Facts about the world a slice ran against, independent of any compile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observations {
    pub world_id: String,
    pub query_id: String,
    pub world_digest: String,
    pub total_facts: usize,
    pub total_factors: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compiled: Option<CompiledObservation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refused: Option<RefusalObservation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph_walk: Option<GraphWalkObservation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle: Option<BundleObservation>,
}

/// The result of executing one vertical slice.
///
/// A report is a claim plus its evidence plus the ways the evidence failed to support it.
/// `failures` being non-empty is not an error condition in the Rust sense — it is the finding —
/// which is why running a slice returns `Ok` with a failing report rather than `Err`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SliceReport {
    pub slice_id: String,
    pub title: String,
    pub demonstrates: Property,
    pub also_exercises: Vec<Property>,
    pub blueprint_modules: Vec<String>,
    pub observations: Observations,
    /// One line per expectation the run did not meet. Empty means the claim still holds.
    pub failures: Vec<String>,
    /// Digest over every field above. Byte-identical across runs by construction.
    pub digest: String,
}

impl SliceReport {
    /// Whether the property this slice claims to demonstrate still holds.
    pub fn holds(&self) -> bool {
        self.failures.is_empty()
    }

    /// Every property this run exercised, primary first.
    pub fn exercised(&self) -> Vec<Property> {
        let mut all = vec![self.demonstrates];
        all.extend(self.also_exercises.iter().copied());
        all
    }

    /// The report body: every field except the digest taken over it.
    pub fn body(&self) -> Value {
        let mut value = serde_json::to_value(self).expect("slice report is serialisable");
        if let Some(map) = value.as_object_mut() {
            map.remove("digest");
        }
        value
    }

    /// Recomputes the digest from the body, the way a consumer must before trusting a report it
    /// did not produce.
    pub fn recompute_digest(&self) -> Result<String, CanonicalError> {
        ContentHash::of_value(&self.body()).map(|hash| hash.as_str().to_string())
    }

    /// Whether the embedded digest matches the body.
    pub fn digest_is_intact(&self) -> bool {
        self.recompute_digest()
            .is_ok_and(|recomputed| recomputed == self.digest)
    }
}
