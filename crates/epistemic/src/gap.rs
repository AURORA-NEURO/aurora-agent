//! What `fiber-query/0.1` would have to carry for 43.50 to reach the compiler.
//!
//! Everything in [`crate::ratedistortion`] and [`crate::voi`] takes the decision loss and the
//! permitted actions as arguments. That is not an API preference. `bioprism-fiber`'s own
//! `Query::missing_contract_fields` reports `permitted_actions` and `decision_loss` as absent from
//! every v0.1 query, and `bioprism-examples` records the consequence twice:
//! `decision_equivalence_quotient` is blocked because "fiber-query/0.1 carries neither
//! permitted_actions nor decision_loss", and `rate_distortion_optimisation` is blocked by "the
//! same missing decision_loss field: there is no loss to trade distortion against".
//!
//! This module is the other half of that sentence. It names the fields, their shapes, and what
//! each one unblocks, so the gap is a diff rather than a complaint.
//!
//! ## The finding that is worse than the gap
//!
//! `fiber-query/0.1` **silently discards** a `decision_loss` field. Its parser reads the keys it
//! knows and ignores the rest, so a caller who writes the field gets no error, no warning, and no
//! effect — and `missing_contract_fields` still reports the field as missing, correctly, while the
//! document on disk appears to supply it. A schema that rejected unknown keys would turn this into
//! a loud failure at the boundary. [`unknown_fields_are_discarded`] demonstrates it against the
//! real parser, and it is the single cheapest thing to fix in this whole area.
//!
//! ## Why this is a schema bump and not an extension
//!
//! Adding `decision_loss` changes what a *conforming consumer* must do: a v0.1 reader that ignores
//! it will compute a context optimised against no loss and label it decision-sufficient. That is
//! the failure 43.50 exists to prevent, so the field cannot ship as optional-and-ignorable. It
//! needs a version in which absence is an error for any pass that consumes it — which is what
//! [`PROPOSED_SCHEMA_VERSION`] denotes and what this crate is not entitled to mint.

use crate::error::EpistemicError;
use serde::Serialize;
use serde_json::{json, Value};

/// The version a query carrying a decision contract would have to declare.
///
/// This crate does not own the wire format and does not ship this version. The constant exists so
/// the proposal has a name that a reviewer can accept or reject, rather than being a paragraph.
pub const PROPOSED_SCHEMA_VERSION: &str = "fiber-query/0.2";

/// The version the workspace actually ships, restated so the two can be compared in one place.
pub const CURRENT_SCHEMA_VERSION: &str = bioprism_fiber::QUERY_SCHEMA_VERSION;

/// One field the current wire format lacks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RequiredField {
    /// Key at the top level of the query document.
    pub name: &'static str,
    /// The shape a conforming document must supply, in prose precise enough to implement from.
    pub shape: &'static str,
    /// What the compiler could compute once the field exists.
    pub unblocks: &'static [&'static str],
    /// Blueprint modules that define a pass against this field.
    pub required_by: &'static [&'static str],
    /// What a pass must do when the field is absent. Never "assume a default".
    pub absent_behaviour: &'static str,
}

/// The decision contract `fiber-query/0.1` cannot express.
///
/// Ordered by how much they unblock. The first two are the ones `bioprism-fiber` reports; the
/// third is already carried but optional, and its optionality is its own defect.
pub const REQUIRED_FOR_RATE_DISTORTION: &[RequiredField] = &[
    RequiredField {
        name: "decision_loss",
        shape: "object with `actions: [string]`, `models: [string]`, and `loss: [[number]]` in \
                actions x models row-major order; every entry a finite real. Optionally \
                `sense: \"loss\" | \"utility\"`, defaulting to loss.",
        unblocks: &[
            "rate-distortion optimisation of the context against decision regret",
            "value of information for evidence actions",
            "decision-equivalence quotienting of the world",
            "a numeric bound on the distortion an omission group can cause, which is the only \
             thing that lets an omission be classified Bounded rather than Unknown",
        ],
        required_by: &["43.50", "43.12", "43.10", "43.26"],
        absent_behaviour: "every pass defined against the loss refuses to run and declares itself \
                           deferred; no pass substitutes a uniform or zero-one loss",
    },
    RequiredField {
        name: "permitted_actions",
        shape: "array of stable action identifiers, a subset of `decision_loss.actions`, naming \
                the actions this query is allowed to distinguish between",
        unblocks: &[
            "restricting the quotient and the distortion to actions the caller may actually take",
            "detecting that a query's forbidden outputs and its loss matrix disagree",
        ],
        required_by: &["43.50", "43.10", "43.32"],
        absent_behaviour: "the action set defaults to nothing, and any pass needing one refuses; \
                           defaulting to all actions would silently widen the decision boundary a \
                           research-only query pattern exists to narrow",
    },
    RequiredField {
        name: "compatible_models",
        shape: "object with `prior: [number]` over `decision_loss.models`, or \
                `set: [string]` naming models with no distribution over them, plus \
                `floor: number` giving the posterior mass below which a model is ruled out",
        unblocks: &[
            "the minimax criterion, and therefore the abstention 43.50 requires when \
             identification fails",
            "reporting identification status instead of a point answer",
        ],
        required_by: &["43.50"],
        absent_behaviour: "only the Bayes criterion is available and identification status is \
                           unreportable; a minimax over an undeclared model set is a minimax over \
                           the library author's guess",
    },
    RequiredField {
        name: "distortion_tolerance",
        shape: "already present and already a number; the defect is that it is optional, so a \
                query can request a compressed context without stating what loss it will accept",
        unblocks: &[
            "the epsilon in epsilon-decision-sufficiency, without which 'sufficient' has no \
             referent",
        ],
        required_by: &["43.50", "43.28"],
        absent_behaviour: "bioprism-fiber already reports this one as missing when unset, which \
                           is the correct behaviour and the model for the other three",
    },
];

/// A query document conforming to the proposed version, for a reviewer to read.
///
/// Deterministic and free of any clock or identifier this crate would have to invent beyond the
/// example's own. Not parseable by `bioprism-fiber`: that is the point of a version bump.
pub fn proposed_query_document() -> Value {
    json!({
        "schema_version": PROPOSED_SCHEMA_VERSION,
        "query_id": "q-molecular-assay-selection-0001",
        "targets": ["assay_choice_supported"],
        "protected_tags": ["specimen", "detection_limit", "consent"],
        "decision_time": "2026-01-01T00:00:00Z",
        "budgets": { "max_facts": 24 },
        "distortion_tolerance": 0.05,
        "decision_loss": {
            "sense": "loss",
            "actions": ["order_panel", "order_single_gene", "defer"],
            "models": ["alteration_present", "alteration_absent"],
            "loss": [
                [0.2, 0.6],
                [0.9, 0.1],
                [0.5, 0.5]
            ]
        },
        "permitted_actions": ["order_panel", "order_single_gene", "defer"],
        "compatible_models": {
            "prior": [0.3, 0.7],
            "floor": 0.01
        }
    })
}

/// The state of one proposed field against a real query document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum FieldState {
    /// The document does not carry the key at all.
    Absent { field: &'static str },
    /// The document carries the key and the compiler ignores it. The dangerous state.
    PresentAndDiscarded { field: &'static str },
    /// The document carries the key and `bioprism-fiber` reads it.
    PresentAndRead { field: &'static str },
}

/// Audits a raw query document against the proposed decision contract.
///
/// Takes the raw JSON rather than a parsed `Query` because the interesting states are the ones the
/// parser erases: a key the parser drops is invisible in the parsed value by construction.
pub fn audit(document: &Value) -> Result<Vec<FieldState>, EpistemicError> {
    let map = document
        .as_object()
        .ok_or_else(|| EpistemicError::QueryRejected {
            schema: CURRENT_SCHEMA_VERSION.to_string(),
            detail: "a query document must be a JSON object".to_string(),
        })?;
    Ok(REQUIRED_FOR_RATE_DISTORTION
        .iter()
        .map(|field| {
            let present = map.contains_key(field.name);
            match (present, read_by_fiber(field.name)) {
                (false, _) => FieldState::Absent { field: field.name },
                (true, true) => FieldState::PresentAndRead { field: field.name },
                (true, false) => FieldState::PresentAndDiscarded { field: field.name },
            }
        })
        .collect())
}

/// Whether `bioprism-fiber`'s v0.1 parser reads a key at all.
///
/// Hand-maintained against `bioprism_fiber::qir`, which reads `schema_version`, `query_id`,
/// `targets`, `protected_tags`, `decision_time`, `budgets`, `role`, `policy`,
/// `distortion_tolerance` and `goal`, and drops everything else without complaint.
fn read_by_fiber(field: &str) -> bool {
    matches!(field, "distortion_tolerance")
}

/// Demonstrates the silent-discard defect against the real parser.
///
/// Returns the fields that were written into a valid v0.1 document, accepted without error, and
/// still reported missing by `bioprism-fiber` afterwards. A non-empty result is the finding.
pub fn unknown_fields_are_discarded() -> Result<Vec<String>, EpistemicError> {
    let mut document = json!({
        "schema_version": CURRENT_SCHEMA_VERSION,
        "query_id": "q-silent-discard-0001",
        "targets": ["assay_choice_supported"],
        "decision_time": "2026-01-01T00:00:00Z",
        "budgets": { "max_facts": 24 },
        "distortion_tolerance": 0.05
    });
    let proposed = proposed_query_document();
    for field in REQUIRED_FOR_RATE_DISTORTION {
        if field.name == "distortion_tolerance" {
            continue;
        }
        if let Some(value) = proposed.get(field.name) {
            document[field.name] = value.clone();
        }
    }

    let query =
        bioprism_fiber::Query::from_json(document.clone()).map_err(|e| {
            EpistemicError::QueryRejected {
                schema: CURRENT_SCHEMA_VERSION.to_string(),
                detail: e.to_string(),
            }
        })?;

    let still_missing = query.missing_contract_fields();
    Ok(REQUIRED_FOR_RATE_DISTORTION
        .iter()
        .filter(|field| document.get(field.name).is_some())
        .filter(|field| still_missing.contains(&field.name))
        .map(|field| field.name.to_string())
        .collect())
}
