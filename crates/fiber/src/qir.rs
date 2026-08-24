//! Query IR: the typed decision contract.
//!
//! Blueprint 43.02 defines a query as `q = ⟨Y, A, t, ω, ℓ, B, ε, R⟩` — targets, permitted
//! actions, time cut, role/policy, decision loss, budget, tolerated distortion and requested
//! outputs. The legacy `fiber-query/0.1` and `fiber-query/0.2` wire forms carry only targets,
//! protected tags, decision time, budgets, role, policy, distortion tolerance and goal.
//!
//! `fiber-query/0.3` is the first executable decision-contract form. It adds a bounded rectangular
//! `decision_loss` matrix, its action/model labels, an explicit `loss` or `utility` sense, and the
//! caller's `permitted_actions`. The parser converts utility to loss exactly once and validates
//! shape, uniqueness, finite values and identifier bounds before FIBER invokes any pass.
//!
//! `fiber-query/0.4` adds the observed-evidence contract needed by rate--distortion: a normalized
//! model prior, an ordered bounded evidence pool of likelihood profiles and an explicit
//! compatible-model floor. The top-level `distortion_tolerance` is required when this contract is
//! present, so FIBER never invents an epsilon or labels a heuristic as a minimum.
//!
//! The permitted-action set `A` and the decision loss `ℓ` are **absent from the legacy wire
//! formats**. That matters: decision-equivalence quotienting (43.10) and rate-distortion
//! optimisation (43.12) are both defined relative to `A` and `ℓ`, so neither can be implemented
//! against 0.2 without extending the schema. [`Query::missing_contract_fields`] reports the gap
//! rather than letting a later pass quietly substitute a default. 0.3 closes only the 43.10
//! input gap; compatible-model posteriors and evidence-pool likelihood bindings for 43.12 are
//! supplied only by 0.4 and remain reported as deferred for older forms. `fiber-query/0.5`
//! adds a separate unperformed-acquisition contract: a normalized prior, complete per-model
//! outcome partitions, a scalarized path budget and a finite horizon.
//!
//! Of the role/policy pair `ω`, only [`Query::policy`] is read — by [`crate::policy`], as the set
//! of obligations the caller accepts. [`Query::role`] is still parsed and discarded: 43.33 binds
//! role and purpose to the query at step 1, `bioprism-scope` registers `role` and `visibility` as
//! policy-class dimensions, and no pass in this crate consults either. That is the same defect the
//! §40 audit found in `policy`, one field over, and it is recorded here rather than fixed silently.
//!
//! # 0.2: an undeclared key is refused
//!
//! Through 0.1 this parser read the keys it knew and dropped the rest. That is cheap for a format
//! nobody hashes, and wrong for this one, because the *whole query document* is hashed into
//! `source_hashes.query_sha256` and thence into the certificate's own digest. An undeclared key
//! was therefore ignored semantically and honoured cryptographically: it could move a compiled
//! answer's identity without being able to move the answer. [`FiberError::UnknownQueryFields`]
//! closes both halves, and [`QUERY_FIELD_PATHS`] is the accepted set the refusal quotes.
//!
//! Note what did *not* change: no field was added, removed or retyped. The only thing 0.2 moves is
//! the reader's declared handling of a key nobody declared — `bioprism-governance` calls that a
//! `CompatibilityMode` change, classifies it `Breaking` on its own terms, and reports zero
//! digest-affecting changes. Both halves of that are load-bearing here and both are asserted in
//! `tests/unknown_query_keys.rs`.
//!
//! # Why a 0.1 label is still read
//!
//! [`ACCEPTED_QUERY_SCHEMA_VERSIONS`] holds five entries, which looks like a loophole and is the
//! opposite. The published `schemas/fiber-v0.1/query.schema.json` has declared
//! `additionalProperties: false` since 0.1: a document carrying an undeclared key was *never* a
//! valid 0.1 query, the parser simply was not enforcing the schema it published. So no document
//! that was ever valid is invalidated by this change, and re-labelling every existing query to 0.2
//! would move `query_sha256` — and therefore the certificate digest — of documents whose content
//! did not change, to record a change in the reader. The reference pair in `fixtures/fiber-v0.1/`
//! is precisely such a document: it is the frozen artifact three implementations agree on, and a
//! 0.2 reader has to be able to replay it or the parity anchor is gone.
//!
//! # Why `fiber-world/0.1` is a separate change
//!
//! It has the identical hole, and worse: [`bioprism_world::World::from_json`] reads named keys and
//! retains the raw document *deliberately*, because certificate hashes are taken over the original
//! bytes. So a world with an undeclared key also compiles to a different certificate digest with
//! identical content. It is not fixed here, for three reasons.
//!
//! First, it is a different named format, so it is a second lineage: `SchemaId::successor_of`
//! refuses to compare `fiber-query` with `fiber-world`, and one classification cannot cover both.
//! Second, `World::validate_reference_compat` is documented as "deliberately no stricter" than the
//! CPython `validate_world`, and its dependent set — `worldgen`, `bioworlds`, `oncoworlds`,
//! `packs`, `store`, `backends` and every world fixture in the tree — is an order of magnitude
//! larger than the query's. Third, and decisively, a query is flat while a world is not: `scope`
//! is an open extension point that `bioprism-scope` registers dimensions into, so "undeclared key"
//! inside a fact's scope needs a design decision this change has no basis to make. Two changes,
//! and this is the first.

use crate::error::FiberError;
use bioprism_epistemic::adaptive::{MAX_ADAPTIVE_ACQUISITIONS, MAX_ADAPTIVE_STEPS};
use bioprism_epistemic::{
    Acquisition, Belief, DecisionProblem, DistortionCriterion, EpistemicError, EvidenceItem,
    EvidencePool, Outcome,
};
use bioprism_ids::{QueryId, VariableName};
use bioprism_scope::Timestamp;
use serde_json::{Map, Value};
use std::collections::BTreeSet;

const MAX_DECISION_CONTRACT_ITEMS: usize = 1_000;
const MAX_DECISION_IDENTIFIER_BYTES: usize = 256;
const MAX_RATE_DISTORTION_EVIDENCE_ITEMS: usize = 16;

/// The version a query this compiler emits or prefers declares.
pub const QUERY_SCHEMA_VERSION: &str = "fiber-query/0.2";

/// The first wire version whose decision contract is mandatory and executable by FIBER.
pub const QUERY_DECISION_SCHEMA_VERSION: &str = "fiber-query/0.3";

/// The first wire version whose decision contract also binds observed evidence for 43.12.
pub const QUERY_RATE_DISTORTION_SCHEMA_VERSION: &str = "fiber-query/0.4";

/// The first wire version whose decision contract also binds an exact adaptive acquisition plan.
pub const QUERY_ADAPTIVE_SCHEMA_VERSION: &str = "fiber-query/0.5";

/// Every version label this parser reads, newest last. See the module docs for why 0.1 is here.
pub const ACCEPTED_QUERY_SCHEMA_VERSIONS: &[&str] = &[
    "fiber-query/0.1",
    QUERY_SCHEMA_VERSION,
    QUERY_DECISION_SCHEMA_VERSION,
    QUERY_RATE_DISTORTION_SCHEMA_VERSION,
    QUERY_ADAPTIVE_SCHEMA_VERSION,
];

/// Every key `fiber-query/0.2` declares, as dotted paths, sorted.
///
/// Sorted because this array is quoted verbatim in [`FiberError::UnknownQueryFields`], and an
/// error message whose contents depend on declaration order is a diff nobody can review. Dotted
/// because `budgets` is the format's one nested object and the published schema closes it too, so
/// `budgets.max_items` has to be refusable by a name that says where it was found.
///
/// `tests/unknown_query_keys.rs` holds this against `schemas/fiber-v0.1/query.schema.json` as an
/// equality, the way `bioprism-governance`'s `known.rs` does for the certificate. A key the parser
/// accepts and the published schema omits is the same defect in the other direction.
pub const QUERY_FIELD_PATHS: &[&str] = &[
    "budgets",
    "budgets.max_facts",
    "budgets.max_tokens",
    "decision_time",
    "distortion_tolerance",
    "goal",
    "policy",
    "protected_tags",
    "query_id",
    "role",
    "schema_version",
    "targets",
];

/// Every key `fiber-query/0.3` declares, including the decision contract consumed by 43.10.
///
/// The 0.2 set remains separately exported because its accepted-key refusal is part of the
/// published compatibility contract and because a 0.2 caller must not accidentally gain 0.3
/// semantics merely by passing through this parser.
pub const QUERY_DECISION_FIELD_PATHS: &[&str] = &[
    "budgets",
    "budgets.max_facts",
    "budgets.max_tokens",
    "decision_loss",
    "decision_loss.actions",
    "decision_loss.loss",
    "decision_loss.models",
    "decision_loss.sense",
    "decision_time",
    "distortion_tolerance",
    "goal",
    "permitted_actions",
    "policy",
    "protected_tags",
    "query_id",
    "role",
    "schema_version",
    "targets",
];

/// Every key `fiber-query/0.4` declares. The nested object and array member shapes are checked by
/// [`parse_rate_distortion_contract`] because dotted paths cannot express an array item's object
/// keys without making the refusal vocabulary unstable.
pub const QUERY_RATE_DISTORTION_FIELD_PATHS: &[&str] = &[
    "budgets",
    "budgets.max_facts",
    "budgets.max_tokens",
    "decision_loss",
    "decision_loss.actions",
    "decision_loss.loss",
    "decision_loss.models",
    "decision_loss.sense",
    "decision_time",
    "distortion_tolerance",
    "goal",
    "permitted_actions",
    "policy",
    "protected_tags",
    "query_id",
    "rate_distortion",
    "rate_distortion.compatibility_floor",
    "rate_distortion.criterion",
    "rate_distortion.evidence_pool",
    "rate_distortion.evidence_pool.items",
    "rate_distortion.prior",
    "role",
    "schema_version",
    "targets",
];

/// Every key `fiber-query/0.5` declares. The nested acquisition and outcome member shapes are
/// checked by [`parse_adaptive_acquisition_contract`] so array-item errors remain contextual.
pub const QUERY_ADAPTIVE_FIELD_PATHS: &[&str] = &[
    "adaptive_acquisition",
    "adaptive_acquisition.acquisitions",
    "adaptive_acquisition.budget",
    "adaptive_acquisition.max_steps",
    "adaptive_acquisition.prior",
    "budgets",
    "budgets.max_facts",
    "budgets.max_tokens",
    "decision_loss",
    "decision_loss.actions",
    "decision_loss.loss",
    "decision_loss.models",
    "decision_loss.sense",
    "decision_time",
    "distortion_tolerance",
    "goal",
    "permitted_actions",
    "policy",
    "protected_tags",
    "query_id",
    "role",
    "schema_version",
    "targets",
];

/// The goal string the CPython reference hard-codes into every Decision Section.
///
/// Hard-coding a radiogenomic goal inside a general compiler is a defect in the reference. A
/// query may supply its own `goal`; when it does not, this value is used so that sections remain
/// byte-comparable with the reference.
pub const REFERENCE_GOAL: &str =
    "Determine whether the proposed radiogenomic split supports a valid external-generalization analysis.";

/// The goal echoed when a post-reference query declares none.
///
/// `fiber-query/0.1` and `/0.2` are the versions the CPython reference accepts, so for those the
/// radiogenomic [`REFERENCE_GOAL`] substitution is reproduced byte for byte. The reference does
/// not accept any later version, so no parity claim reaches them, and substituting another
/// domain's sentence there would be an invention rather than a compatibility: a missing goal is
/// reported as missing.
pub const NO_DECLARED_GOAL: &str = "The query declares no goal.";

/// Whether the query's matrix is written as a loss to minimise or a utility to maximise.
///
/// The epistemic kernel uses loss internally. Utility input is negated exactly once at the QIR
/// boundary, so every downstream pass has one convention and the wire contract remains explicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionSense {
    Loss,
    Utility,
}

impl DecisionSense {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Loss => "loss",
            Self::Utility => "utility",
        }
    }
}

/// The executable decision contract carried by `fiber-query/0.3`.
///
/// Blueprint 43.02 names `A` and `ℓ` as query inputs, and 43.10 quotients only distinctions that
/// cannot change an allowed decision. The wire parser therefore requires both instead of
/// widening a research query to every action or inventing a loss. Compatible-model posterior and
/// evidence-pool bindings are supplied by the sibling 0.4 [`RateDistortionContract`]; 43.10 needs
/// only this exact model/action loss table.
#[derive(Debug, Clone, PartialEq)]
pub struct DecisionContract {
    pub problem: DecisionProblem,
    pub permitted_actions: Vec<String>,
    pub sense: DecisionSense,
}

/// The observed-evidence inputs to the exhaustive decision-relative context compiler.
///
/// `prior` is the caller's normalized mass over the decision contract's models. Each evidence
/// item is already observed; its likelihood vector says how conditioning on retaining that item
/// updates the prior. This is deliberately not an acquisition model and cannot be used to claim
/// value for an unperformed test.
#[derive(Debug, Clone, PartialEq)]
pub struct RateDistortionContract {
    pub prior: Belief,
    pub evidence_pool: EvidencePool,
    pub criterion: DistortionCriterion,
    pub compatibility_floor: f64,
}

/// The caller-declared inputs to an exact finite-horizon adaptive acquisition policy.
///
/// Unlike the observed `RateDistortionContract`, these outcomes describe tests that have not
/// been performed. The compiler may plan against them, but the resulting trace never asserts an
/// observation receipt, provider execution, authorization, or causal identification.
#[derive(Debug, Clone, PartialEq)]
pub struct AdaptiveAcquisitionContract {
    pub prior: Belief,
    pub acquisitions: Vec<Acquisition>,
    pub budget: f64,
    pub max_steps: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Budgets {
    pub max_facts: usize,
    pub max_tokens: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct Query {
    pub query_id: QueryId,
    pub targets: Vec<VariableName>,
    pub protected_tags: BTreeSet<String>,
    pub decision_time: Timestamp,
    /// The decision time exactly as written, echoed into the section verbatim.
    pub decision_time_raw: String,
    pub budgets: Budgets,
    pub role: Option<String>,
    pub policy: Vec<String>,
    pub distortion_tolerance: Option<f64>,
    pub goal: Option<String>,
    pub decision_contract: Option<DecisionContract>,
    pub rate_distortion: Option<RateDistortionContract>,
    pub adaptive_acquisition: Option<AdaptiveAcquisitionContract>,
    schema_version: String,
    raw: Value,
}

impl Query {
    pub fn from_json(raw: Value) -> Result<Self, FiberError> {
        let map = raw.as_object().ok_or(FiberError::QueryNotAnObject)?;

        let schema_version = map
            .get("schema_version")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !ACCEPTED_QUERY_SCHEMA_VERSIONS.contains(&schema_version) {
            return Err(FiberError::UnsupportedQuerySchema {
                expected: ACCEPTED_QUERY_SCHEMA_VERSIONS,
                actual: schema_version.to_string(),
            });
        }

        let accepted_fields = if schema_version == QUERY_ADAPTIVE_SCHEMA_VERSION {
            QUERY_ADAPTIVE_FIELD_PATHS
        } else if schema_version == QUERY_RATE_DISTORTION_SCHEMA_VERSION {
            QUERY_RATE_DISTORTION_FIELD_PATHS
        } else if schema_version == QUERY_DECISION_SCHEMA_VERSION {
            QUERY_DECISION_FIELD_PATHS
        } else {
            QUERY_FIELD_PATHS
        };
        reject_undeclared_fields(map, accepted_fields)?;

        let query_id_text = map
            .get("query_id")
            .and_then(Value::as_str)
            .ok_or(FiberError::MissingQueryField("query_id"))?;
        let query_id = QueryId::parse(query_id_text)
            .map_err(|e| FiberError::InvalidIdentifier(e.to_string()))?;

        let targets_raw = map
            .get("targets")
            .and_then(Value::as_array)
            .ok_or(FiberError::MissingQueryField("targets"))?;
        let targets = targets_raw
            .iter()
            .map(|item| {
                item.as_str()
                    .ok_or(FiberError::WrongQueryFieldType {
                        field: "targets",
                        expected: "array of strings",
                    })
                    .and_then(|name| {
                        VariableName::parse(name)
                            .map_err(|e| FiberError::InvalidIdentifier(e.to_string()))
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let decision_time_raw = map
            .get("decision_time")
            .and_then(Value::as_str)
            .ok_or(FiberError::MissingQueryField("decision_time"))?
            .to_string();
        let decision_time = Timestamp::parse(&decision_time_raw)
            .map_err(|e| FiberError::InvalidDecisionTime(e.to_string()))?;

        let budgets_map = map
            .get("budgets")
            .and_then(Value::as_object)
            .ok_or(FiberError::MissingQueryField("budgets"))?;
        let max_facts = budgets_map
            .get("max_facts")
            .and_then(Value::as_u64)
            .ok_or(FiberError::MissingQueryField("budgets.max_facts"))?
            as usize;

        let decision_contract = if schema_version == QUERY_DECISION_SCHEMA_VERSION
            || schema_version == QUERY_RATE_DISTORTION_SCHEMA_VERSION
            || schema_version == QUERY_ADAPTIVE_SCHEMA_VERSION
        {
            Some(parse_decision_contract(map)?)
        } else {
            None
        };

        let distortion_tolerance = map.get("distortion_tolerance").and_then(Value::as_f64);
        let rate_distortion = if schema_version == QUERY_RATE_DISTORTION_SCHEMA_VERSION {
            let contract = decision_contract
                .as_ref()
                .ok_or(FiberError::MissingQueryField("decision_loss"))?;
            if distortion_tolerance.is_none() {
                return Err(FiberError::MissingQueryField("distortion_tolerance"));
            }
            Some(parse_rate_distortion_contract(map, &contract.problem)?)
        } else {
            None
        };

        let adaptive_acquisition = if schema_version == QUERY_ADAPTIVE_SCHEMA_VERSION {
            let contract = decision_contract
                .as_ref()
                .ok_or(FiberError::MissingQueryField("decision_loss"))?;
            Some(parse_adaptive_acquisition_contract(map, &contract.problem)?)
        } else {
            None
        };

        Ok(Query {
            query_id,
            targets,
            protected_tags: string_set(map.get("protected_tags")),
            decision_time,
            decision_time_raw,
            budgets: Budgets {
                max_facts,
                max_tokens: budgets_map
                    .get("max_tokens")
                    .and_then(Value::as_u64)
                    .map(|v| v as usize),
            },
            role: map.get("role").and_then(Value::as_str).map(str::to_string),
            policy: string_set(map.get("policy")).into_iter().collect(),
            distortion_tolerance,
            goal: map.get("goal").and_then(Value::as_str).map(str::to_string),
            decision_contract,
            rate_distortion,
            adaptive_acquisition,
            schema_version: schema_version.to_string(),
            raw,
        })
    }

    pub fn raw(&self) -> &Value {
        &self.raw
    }

    /// The wire version this query was parsed from.
    pub fn schema_version(&self) -> &str {
        &self.schema_version
    }

    /// The goal echoed into the Decision Section.
    ///
    /// The CPython reference accepts only `fiber-query/0.1` and `/0.2` and hard-codes
    /// [`REFERENCE_GOAL`] into every section without reading this key; that substitution is
    /// reproduced for those two versions so sections stay byte-comparable, and it is a recorded
    /// defect in the reference. Later versions carry no parity claim, so a missing goal is
    /// reported as [`NO_DECLARED_GOAL`] rather than replaced by another domain's sentence.
    pub fn goal_text(&self) -> &str {
        match &self.goal {
            Some(goal) => goal,
            None if self.schema_version == "fiber-query/0.1"
                || self.schema_version == QUERY_SCHEMA_VERSION =>
            {
                REFERENCE_GOAL
            }
            None => NO_DECLARED_GOAL,
        }
    }

    pub fn has_decision_contract(&self) -> bool {
        self.decision_contract.is_some()
    }

    pub fn has_rate_distortion_contract(&self) -> bool {
        self.rate_distortion.is_some()
    }

    pub fn has_adaptive_acquisition_contract(&self) -> bool {
        self.adaptive_acquisition.is_some()
    }

    /// Components of the formal decision contract that this query cannot express.
    ///
    /// Any pass that needs one of these must refuse to run rather than assume a default. Legacy
    /// Legacy 0.1/0.2 queries return the two decision fields; 0.3, 0.4 and 0.5 carry an explicit
    /// decision contract. Distortion tolerance is independent and remains optional except for
    /// the observed rate-distortion contract.
    pub fn missing_contract_fields(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        if self.decision_contract.is_none() {
            missing.extend(["permitted_actions", "decision_loss"]);
        }
        if self.distortion_tolerance.is_none() {
            missing.push("distortion_tolerance");
        }
        missing
    }
}

/// Refuses a query carrying any key outside the accepted path set for its version.
///
/// Runs immediately after the version check and before any field is read, so a caller is told
/// their document is not a query of this format before being told which of its fields is missing.
/// Reporting `missing required query field "targets"` about a document that also carries a
/// misspelled `target` would send them after the wrong problem.
///
/// `budgets` is descended into and nothing else is, because it is the only nested object the
/// format declares. `policy`, `protected_tags` and `targets` are arrays of strings with no
/// interior to declare, and a value that is not an object cannot hide a key.
fn reject_undeclared_fields(
    map: &Map<String, Value>,
    accepted_fields: &'static [&'static str],
) -> Result<(), FiberError> {
    let mut undeclared: Vec<String> = map
        .keys()
        .filter(|key| !accepted_fields.contains(&key.as_str()))
        .cloned()
        .collect();

    if let Some(Value::Object(budgets)) = map.get("budgets") {
        undeclared.extend(
            budgets
                .keys()
                .map(|key| format!("budgets.{key}"))
                .filter(|path| !accepted_fields.contains(&path.as_str())),
        );
    }

    if undeclared.is_empty() {
        return Ok(());
    }
    undeclared.sort();
    Err(FiberError::UnknownQueryFields {
        fields: undeclared,
        accepted: accepted_fields,
    })
}

fn parse_decision_contract(map: &Map<String, Value>) -> Result<DecisionContract, FiberError> {
    let decision_loss = map
        .get("decision_loss")
        .and_then(Value::as_object)
        .ok_or(FiberError::MissingQueryField("decision_loss"))?;
    let allowed = ["actions", "loss", "models", "sense"];
    let unknown: Vec<&str> = decision_loss
        .keys()
        .map(String::as_str)
        .filter(|key| !allowed.contains(key))
        .collect();
    if !unknown.is_empty() {
        return Err(FiberError::InvalidDecisionContract(format!(
            "decision_loss carries undeclared field(s) {unknown:?}"
        )));
    }

    let sense = match decision_loss.get("sense").and_then(Value::as_str) {
        None | Some("loss") => DecisionSense::Loss,
        Some("utility") => DecisionSense::Utility,
        Some(other) => {
            return Err(FiberError::InvalidDecisionContract(format!(
                "sense must be \"loss\" or \"utility\", got {other:?}"
            )))
        }
    };
    let actions = parse_contract_strings(decision_loss, "actions", "decision_loss.actions")?;
    let models = parse_contract_strings(decision_loss, "models", "decision_loss.models")?;
    let loss_value = decision_loss
        .get("loss")
        .and_then(Value::as_array)
        .ok_or(FiberError::MissingQueryField("decision_loss.loss"))?;
    if loss_value.is_empty() || loss_value.len() > MAX_DECISION_CONTRACT_ITEMS {
        return Err(FiberError::InvalidDecisionContract(format!(
            "decision_loss.loss must contain between 1 and {MAX_DECISION_CONTRACT_ITEMS} rows"
        )));
    }
    if loss_value.len() != actions.len() {
        return Err(FiberError::InvalidDecisionContract(format!(
            "decision_loss.loss has {} rows but actions declares {}",
            loss_value.len(),
            actions.len()
        )));
    }
    let mut loss = Vec::with_capacity(actions.len() * models.len());
    for (row_index, row) in loss_value.iter().enumerate() {
        let row = row.as_array().ok_or_else(|| {
            FiberError::InvalidDecisionContract(format!(
                "decision_loss.loss[{row_index}] must be an array of finite numbers"
            ))
        })?;
        if row.len() != models.len() {
            return Err(FiberError::InvalidDecisionContract(format!(
                "decision_loss.loss[{row_index}] has {} columns but models declares {}",
                row.len(),
                models.len()
            )));
        }
        for (model_index, value) in row.iter().enumerate() {
            let value = value.as_f64().ok_or_else(|| {
                FiberError::InvalidDecisionContract(format!(
                    "decision_loss.loss[{row_index}][{model_index}] must be a finite number"
                ))
            })?;
            loss.push(value);
        }
    }
    let problem = match sense {
        DecisionSense::Loss => DecisionProblem::new(actions, models, loss),
        DecisionSense::Utility => DecisionProblem::from_utilities(actions, models, loss),
    }
    .map_err(decision_contract_error)?;

    let permitted_actions = parse_top_level_strings(map, "permitted_actions")?;
    if permitted_actions.is_empty() {
        return Err(FiberError::InvalidDecisionContract(
            "permitted_actions must contain at least one action".into(),
        ));
    }
    let mut seen = BTreeSet::new();
    for action in &permitted_actions {
        if !seen.insert(action.clone()) {
            return Err(FiberError::InvalidDecisionContract(format!(
                "permitted_actions repeats {action:?}"
            )));
        }
        problem
            .action_index(action)
            .map_err(decision_contract_error)?;
    }

    Ok(DecisionContract {
        problem,
        permitted_actions,
        sense,
    })
}

fn parse_rate_distortion_contract(
    map: &Map<String, Value>,
    problem: &DecisionProblem,
) -> Result<RateDistortionContract, FiberError> {
    let object = map
        .get("rate_distortion")
        .and_then(Value::as_object)
        .ok_or(FiberError::MissingQueryField("rate_distortion"))?;
    let allowed = ["compatibility_floor", "criterion", "evidence_pool", "prior"];
    let unknown: Vec<&str> = object
        .keys()
        .map(String::as_str)
        .filter(|key| !allowed.contains(key))
        .collect();
    if !unknown.is_empty() {
        return Err(FiberError::InvalidRateDistortionContract(format!(
            "rate_distortion carries undeclared field(s) {unknown:?}"
        )));
    }

    let criterion = match object.get("criterion").and_then(Value::as_str) {
        Some("bayes_regret") => DistortionCriterion::BayesRegret,
        Some("minimax_regret") => DistortionCriterion::MinimaxRegret,
        Some(other) => {
            return Err(FiberError::InvalidRateDistortionContract(format!(
                "criterion must be \"bayes_regret\" or \"minimax_regret\", got {other:?}"
            )))
        }
        None => return Err(FiberError::MissingQueryField("rate_distortion.criterion")),
    };

    let compatibility_floor = object
        .get("compatibility_floor")
        .and_then(Value::as_f64)
        .ok_or(FiberError::MissingQueryField(
            "rate_distortion.compatibility_floor",
        ))?;
    if !compatibility_floor.is_finite() || !(0.0..=1.0).contains(&compatibility_floor) {
        return Err(FiberError::InvalidRateDistortionContract(
            "compatibility_floor must be finite and between 0 and 1".into(),
        ));
    }

    let prior_values = object
        .get("prior")
        .and_then(Value::as_array)
        .ok_or(FiberError::MissingQueryField("rate_distortion.prior"))?;
    if prior_values.len() != problem.model_count() {
        return Err(FiberError::InvalidRateDistortionContract(format!(
            "prior has {} entries but decision_loss declares {} models",
            prior_values.len(),
            problem.model_count()
        )));
    }
    let prior = prior_values
        .iter()
        .enumerate()
        .map(|(model, value)| {
            value.as_f64().ok_or_else(|| {
                FiberError::InvalidRateDistortionContract(format!(
                    "prior[{model}] must be a finite non-negative number"
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let prior = Belief::new(prior).map_err(rate_distortion_error)?;
    prior
        .check_against(problem)
        .map_err(rate_distortion_error)?;

    let pool_object = object
        .get("evidence_pool")
        .and_then(Value::as_object)
        .ok_or(FiberError::MissingQueryField(
            "rate_distortion.evidence_pool",
        ))?;
    let pool_allowed = ["items"];
    let pool_unknown: Vec<&str> = pool_object
        .keys()
        .map(String::as_str)
        .filter(|key| !pool_allowed.contains(key))
        .collect();
    if !pool_unknown.is_empty() {
        return Err(FiberError::InvalidRateDistortionContract(format!(
            "rate_distortion.evidence_pool carries undeclared field(s) {pool_unknown:?}"
        )));
    }
    let raw_items =
        pool_object
            .get("items")
            .and_then(Value::as_array)
            .ok_or(FiberError::MissingQueryField(
                "rate_distortion.evidence_pool.items",
            ))?;
    if raw_items.len() > MAX_RATE_DISTORTION_EVIDENCE_ITEMS {
        return Err(FiberError::InvalidRateDistortionContract(format!(
            "evidence_pool contains {} items; exhaustive rate-distortion is capped at {MAX_RATE_DISTORTION_EVIDENCE_ITEMS} items",
            raw_items.len()
        )));
    }

    let mut items = Vec::with_capacity(raw_items.len());
    for (index, raw_item) in raw_items.iter().enumerate() {
        let item = raw_item.as_object().ok_or_else(|| {
            FiberError::InvalidRateDistortionContract(format!(
                "evidence_pool.items[{index}] must be an object"
            ))
        })?;
        let item_allowed = ["cost", "id", "likelihood"];
        let item_unknown: Vec<&str> = item
            .keys()
            .map(String::as_str)
            .filter(|key| !item_allowed.contains(key))
            .collect();
        if !item_unknown.is_empty() {
            return Err(FiberError::InvalidRateDistortionContract(format!(
                "evidence_pool.items[{index}] carries undeclared field(s) {item_unknown:?}"
            )));
        }
        let id = item.get("id").and_then(Value::as_str).ok_or_else(|| {
            FiberError::InvalidRateDistortionContract(format!(
                "evidence_pool.items[{index}].id must be a non-empty string"
            ))
        })?;
        if id.trim().is_empty() || id.len() > MAX_DECISION_IDENTIFIER_BYTES {
            return Err(FiberError::InvalidRateDistortionContract(format!(
                "evidence_pool.items[{index}].id must be non-empty and at most {MAX_DECISION_IDENTIFIER_BYTES} bytes"
            )));
        }
        let cost = item.get("cost").and_then(Value::as_f64).ok_or_else(|| {
            FiberError::InvalidRateDistortionContract(format!(
                "evidence_pool.items[{index}].cost must be a finite non-negative number"
            ))
        })?;
        let raw_likelihood = item
            .get("likelihood")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                FiberError::InvalidRateDistortionContract(format!(
                    "evidence_pool.items[{index}].likelihood must be an array of one value per model"
                ))
            })?;
        if raw_likelihood.len() != problem.model_count() {
            return Err(FiberError::InvalidRateDistortionContract(format!(
                "evidence_pool.items[{index}].likelihood has {} entries but decision_loss declares {} models",
                raw_likelihood.len(),
                problem.model_count()
            )));
        }
        let likelihood = raw_likelihood
            .iter()
            .enumerate()
            .map(|(model, value)| {
                value.as_f64().ok_or_else(|| {
                    FiberError::InvalidRateDistortionContract(format!(
                        "evidence_pool.items[{index}].likelihood[{model}] must be a finite non-negative number"
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        items.push(EvidenceItem::new(id, cost, likelihood).map_err(rate_distortion_error)?);
    }
    let evidence_pool = EvidencePool::new(items).map_err(rate_distortion_error)?;
    evidence_pool
        .check_against(problem)
        .map_err(rate_distortion_error)?;

    Ok(RateDistortionContract {
        prior,
        evidence_pool,
        criterion,
        compatibility_floor,
    })
}

fn parse_adaptive_acquisition_contract(
    map: &Map<String, Value>,
    problem: &DecisionProblem,
) -> Result<AdaptiveAcquisitionContract, FiberError> {
    let object = map
        .get("adaptive_acquisition")
        .and_then(Value::as_object)
        .ok_or(FiberError::MissingQueryField("adaptive_acquisition"))?;
    let allowed = ["acquisitions", "budget", "max_steps", "prior"];
    let unknown: Vec<&str> = object
        .keys()
        .map(String::as_str)
        .filter(|key| !allowed.contains(key))
        .collect();
    if !unknown.is_empty() {
        return Err(FiberError::InvalidAdaptiveAcquisitionContract(format!(
            "adaptive_acquisition carries undeclared field(s) {unknown:?}"
        )));
    }

    let budget = object
        .get("budget")
        .and_then(Value::as_f64)
        .ok_or(FiberError::MissingQueryField("adaptive_acquisition.budget"))?;
    if !budget.is_finite() || budget < 0.0 {
        return Err(FiberError::InvalidAdaptiveAcquisitionContract(
            "budget must be finite and non-negative".into(),
        ));
    }

    let max_steps =
        object
            .get("max_steps")
            .and_then(Value::as_u64)
            .ok_or(FiberError::MissingQueryField(
                "adaptive_acquisition.max_steps",
            ))? as usize;
    if max_steps > MAX_ADAPTIVE_STEPS {
        return Err(FiberError::InvalidAdaptiveAcquisitionContract(format!(
            "max_steps is {max_steps}; exact adaptive planning is capped at {MAX_ADAPTIVE_STEPS}"
        )));
    }

    let prior_values = object
        .get("prior")
        .and_then(Value::as_array)
        .ok_or(FiberError::MissingQueryField("adaptive_acquisition.prior"))?;
    if prior_values.len() != problem.model_count() {
        return Err(FiberError::InvalidAdaptiveAcquisitionContract(format!(
            "prior has {} entries but decision_loss declares {} models",
            prior_values.len(),
            problem.model_count()
        )));
    }
    let prior = prior_values
        .iter()
        .enumerate()
        .map(|(model, value)| {
            value.as_f64().ok_or_else(|| {
                FiberError::InvalidAdaptiveAcquisitionContract(format!(
                    "prior[{model}] must be a finite non-negative number"
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let prior = Belief::new(prior).map_err(adaptive_acquisition_error)?;
    prior
        .check_against(problem)
        .map_err(adaptive_acquisition_error)?;

    let raw_acquisitions = object.get("acquisitions").and_then(Value::as_array).ok_or(
        FiberError::MissingQueryField("adaptive_acquisition.acquisitions"),
    )?;
    if raw_acquisitions.is_empty() || raw_acquisitions.len() > MAX_ADAPTIVE_ACQUISITIONS {
        return Err(FiberError::InvalidAdaptiveAcquisitionContract(format!(
            "acquisitions must contain between 1 and {MAX_ADAPTIVE_ACQUISITIONS} items"
        )));
    }

    let mut acquisitions = Vec::with_capacity(raw_acquisitions.len());
    for (index, raw_acquisition) in raw_acquisitions.iter().enumerate() {
        let acquisition = raw_acquisition.as_object().ok_or_else(|| {
            FiberError::InvalidAdaptiveAcquisitionContract(format!(
                "acquisitions[{index}] must be an object"
            ))
        })?;
        let allowed = ["cost", "id", "outcomes"];
        let unknown: Vec<&str> = acquisition
            .keys()
            .map(String::as_str)
            .filter(|key| !allowed.contains(key))
            .collect();
        if !unknown.is_empty() {
            return Err(FiberError::InvalidAdaptiveAcquisitionContract(format!(
                "acquisitions[{index}] carries undeclared field(s) {unknown:?}"
            )));
        }
        let id = acquisition
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                FiberError::InvalidAdaptiveAcquisitionContract(format!(
                    "acquisitions[{index}].id must be a non-empty string"
                ))
            })?;
        if id.trim().is_empty() || id.len() > MAX_DECISION_IDENTIFIER_BYTES {
            return Err(FiberError::InvalidAdaptiveAcquisitionContract(format!(
                "acquisitions[{index}].id must be non-empty and at most {MAX_DECISION_IDENTIFIER_BYTES} bytes"
            )));
        }
        let cost = acquisition
            .get("cost")
            .and_then(Value::as_f64)
            .ok_or_else(|| {
                FiberError::InvalidAdaptiveAcquisitionContract(format!(
                    "acquisitions[{index}].cost must be a finite non-negative number"
                ))
            })?;
        let raw_outcomes = acquisition
            .get("outcomes")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                FiberError::InvalidAdaptiveAcquisitionContract(format!(
                    "acquisitions[{index}].outcomes must be an array"
                ))
            })?;
        if raw_outcomes.is_empty() || raw_outcomes.len() > MAX_DECISION_CONTRACT_ITEMS {
            return Err(FiberError::InvalidAdaptiveAcquisitionContract(format!(
                "acquisitions[{index}].outcomes must contain between 1 and {MAX_DECISION_CONTRACT_ITEMS} items"
            )));
        }
        let mut outcomes = Vec::with_capacity(raw_outcomes.len());
        let mut labels = BTreeSet::new();
        for (outcome_index, raw_outcome) in raw_outcomes.iter().enumerate() {
            let outcome = raw_outcome.as_object().ok_or_else(|| {
                FiberError::InvalidAdaptiveAcquisitionContract(format!(
                    "acquisitions[{index}].outcomes[{outcome_index}] must be an object"
                ))
            })?;
            let allowed = ["label", "likelihood"];
            let unknown: Vec<&str> = outcome
                .keys()
                .map(String::as_str)
                .filter(|key| !allowed.contains(key))
                .collect();
            if !unknown.is_empty() {
                return Err(FiberError::InvalidAdaptiveAcquisitionContract(format!(
                    "acquisitions[{index}].outcomes[{outcome_index}] carries undeclared field(s) {unknown:?}"
                )));
            }
            let label = outcome
                .get("label")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    FiberError::InvalidAdaptiveAcquisitionContract(format!(
                        "acquisitions[{index}].outcomes[{outcome_index}].label must be a string"
                    ))
                })?;
            if label.trim().is_empty() || label.len() > MAX_DECISION_IDENTIFIER_BYTES {
                return Err(FiberError::InvalidAdaptiveAcquisitionContract(format!(
                    "acquisitions[{index}].outcomes[{outcome_index}].label must be non-empty and at most {MAX_DECISION_IDENTIFIER_BYTES} bytes"
                )));
            }
            if !labels.insert(label.to_string()) {
                return Err(FiberError::InvalidAdaptiveAcquisitionContract(format!(
                    "acquisitions[{index}] repeats outcome label {label:?}"
                )));
            }
            let raw_likelihood = outcome
                .get("likelihood")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    FiberError::InvalidAdaptiveAcquisitionContract(format!(
                        "acquisitions[{index}].outcomes[{outcome_index}].likelihood must be an array"
                    ))
                })?;
            if raw_likelihood.len() != problem.model_count() {
                return Err(FiberError::InvalidAdaptiveAcquisitionContract(format!(
                    "acquisitions[{index}].outcomes[{outcome_index}].likelihood has {} entries but decision_loss declares {} models",
                    raw_likelihood.len(),
                    problem.model_count()
                )));
            }
            let likelihood = raw_likelihood
                .iter()
                .enumerate()
                .map(|(model, value)| {
                    value.as_f64().ok_or_else(|| {
                        FiberError::InvalidAdaptiveAcquisitionContract(format!(
                            "acquisitions[{index}].outcomes[{outcome_index}].likelihood[{model}] must be a finite non-negative number"
                        ))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            outcomes.push(Outcome::new(label, likelihood));
        }
        acquisitions.push(
            Acquisition::new(id, cost, outcomes, problem.model_count())
                .map_err(adaptive_acquisition_error)?,
        );
    }
    let ids = acquisitions
        .iter()
        .map(|acquisition| acquisition.id.clone())
        .collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    for id in ids {
        if !seen.insert(id.clone()) {
            return Err(FiberError::InvalidAdaptiveAcquisitionContract(format!(
                "acquisitions repeats id {id:?}"
            )));
        }
    }

    Ok(AdaptiveAcquisitionContract {
        prior,
        acquisitions,
        budget,
        max_steps,
    })
}

fn parse_contract_strings(
    object: &Map<String, Value>,
    key: &'static str,
    field: &'static str,
) -> Result<Vec<String>, FiberError> {
    let value = object
        .get(key)
        .ok_or(FiberError::MissingQueryField(field))?;
    parse_strings(value, field)
}

fn parse_top_level_strings(
    object: &Map<String, Value>,
    field: &'static str,
) -> Result<Vec<String>, FiberError> {
    let value = object
        .get(field)
        .ok_or(FiberError::MissingQueryField(field))?;
    parse_strings(value, field)
}

fn parse_strings(value: &Value, field: &'static str) -> Result<Vec<String>, FiberError> {
    let items = value.as_array().ok_or(FiberError::WrongQueryFieldType {
        field,
        expected: "array of non-empty strings",
    })?;
    if items.len() > MAX_DECISION_CONTRACT_ITEMS {
        return Err(FiberError::InvalidDecisionContract(format!(
            "{field} contains {} items; the maximum is {MAX_DECISION_CONTRACT_ITEMS}",
            items.len()
        )));
    }
    items
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let text = value
                .as_str()
                .ok_or(FiberError::InvalidDecisionContract(format!(
                    "{field}[{index}] must be a string"
                )))?;
            if text.trim().is_empty() {
                return Err(FiberError::InvalidDecisionContract(format!(
                    "{field}[{index}] must not be empty"
                )));
            }
            if text.len() > MAX_DECISION_IDENTIFIER_BYTES {
                return Err(FiberError::InvalidDecisionContract(format!(
                    "{field}[{index}] exceeds the {MAX_DECISION_IDENTIFIER_BYTES}-byte limit"
                )));
            }
            Ok(text.to_string())
        })
        .collect()
}

fn decision_contract_error(error: EpistemicError) -> FiberError {
    FiberError::InvalidDecisionContract(error.to_string())
}

fn rate_distortion_error(error: EpistemicError) -> FiberError {
    FiberError::InvalidRateDistortionContract(error.to_string())
}

fn adaptive_acquisition_error(error: EpistemicError) -> FiberError {
    FiberError::InvalidAdaptiveAcquisitionContract(error.to_string())
}

fn string_set(value: Option<&Value>) -> BTreeSet<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}
