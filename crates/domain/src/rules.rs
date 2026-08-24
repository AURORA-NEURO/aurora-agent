//! The rule language: declarative violation checks over the compiled value map.
//!
//! A rule oracle is the 43.41 contract in data: deterministic, witness-producing, score-free.
//! Checks are *violation detectors* — a fired check is evidence against the world, exactly as a
//! leakage witness is — and the verdict falls out of what ran:
//!
//! * any fired check → `invalid`, carrying one witness per fired check;
//! * no fired check but a check that could not run → `underdetermined`, carrying one witness
//!   per unrun check naming the variable that stopped it;
//! * everything ran and nothing fired → `valid`.
//!
//! `invalid` outranks `underdetermined` because a proven violation stands even when another
//! check is blind; an `invalid` verdict still reports its unrun checks, after the violations,
//! so the gap is never hidden behind the finding.
//!
//! Predicates evaluate under strong three-valued logic: a conjunction with one `false` limb is
//! determinately `false` whatever the other limbs would have said, a disjunction with one
//! `true` limb is determinately `true`, and otherwise an unevaluable limb makes the whole
//! predicate unevaluable. A predicate over an absent or wrongly-typed variable is *unevaluable*,
//! never `false`: "the check did not run" and "the check passed" must not share a
//! representation. Only [`Predicate::Exists`] and [`Predicate::Missing`] are total, because
//! absence is the very thing they ask about.

use crate::DomainError;
use bioprism_fiber::{DecisionOracle, FiberError};
use bioprism_ids::to_canonical_string;
use bioprism_section::{LeakageWitness, OracleVerdict};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// One condition over the compiled value map.
#[derive(Debug, Clone, PartialEq)]
pub enum Predicate {
    /// Total: true when the variable was delivered, whatever its value.
    Exists { variable: String },
    /// Total: true when the variable was not delivered.
    Missing { variable: String },
    Equals { variable: String, value: Value },
    NotEquals { variable: String, value: Value },
    NumberAtLeast { variable: String, minimum: f64 },
    NumberBelow { variable: String, maximum: f64 },
    /// Lexicographic on the raw strings, matching the reference oracle's flagged temporal
    /// comparison; refused for non-strings rather than coerced.
    StringBefore { variable: String, than: String },
    StringAfter { variable: String, than: String },
    /// True when the array-valued variable contains the given element.
    Contains { variable: String, value: Value },
    /// True when the object-valued variable carries the given key.
    HasKey { variable: String, key: String },
    /// True when the array- or object-valued variable has at least one element.
    Nonempty { variable: String },
    /// True when the array- or object-valued variable has at least `minimum` elements.
    CountAtLeast { variable: String, minimum: usize },
    AllOf { predicates: Vec<Predicate> },
    AnyOf { predicates: Vec<Predicate> },
    Not { predicate: Box<Predicate> },
}

/// Why a predicate could not be evaluated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Obstruction {
    pub variable: String,
    pub reason: String,
}

impl Obstruction {
    fn absent(variable: &str) -> Obstruction {
        Obstruction {
            variable: variable.to_string(),
            reason: "absent from the compiled value map".into(),
        }
    }

    fn wrong_type(variable: &str, expected: &str, found: &Value) -> Obstruction {
        Obstruction {
            variable: variable.to_string(),
            reason: format!("expected {expected}, found {}", type_name(found)),
        }
    }
}

fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

impl Predicate {
    /// Parses the strict wire form: an object with a `kind` and exactly the fields that kind
    /// declares. An undeclared field is refused before a missing one is reported, for the same
    /// reason the query parser refuses unknown keys first: a misspelled field must not send the
    /// author after the wrong problem.
    pub fn from_json(document: &Value) -> Result<Predicate, DomainError> {
        let map = document
            .as_object()
            .ok_or_else(|| rule_error("predicate is not an object"))?;
        let kind = map
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| rule_error("predicate declares no \"kind\""))?;

        let declared: &[&str] = match kind {
            "exists" | "missing" | "nonempty" => &["kind", "variable"],
            "equals" | "not_equals" | "contains" => &["kind", "variable", "value"],
            "number_at_least" => &["kind", "variable", "minimum"],
            "number_below" => &["kind", "variable", "maximum"],
            "string_before" | "string_after" => &["kind", "variable", "than"],
            "has_key" => &["kind", "variable", "key"],
            "count_at_least" => &["kind", "variable", "minimum"],
            "all_of" | "any_of" => &["kind", "predicates"],
            "not" => &["kind", "predicate"],
            other => {
                return Err(rule_error(&format!(
                    "unknown predicate kind {other:?}"
                )))
            }
        };
        if let Some(unknown) = map.keys().find(|key| !declared.contains(&key.as_str())) {
            return Err(rule_error(&format!(
                "undeclared field {unknown:?} on predicate kind {kind:?}"
            )));
        }

        let variable = |field: &'static str| -> Result<String, DomainError> {
            map.get(field)
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| {
                    rule_error(&format!("predicate kind {kind:?} needs a string {field:?}"))
                })
        };
        let number = |field: &'static str| -> Result<f64, DomainError> {
            map.get(field).and_then(Value::as_f64).ok_or_else(|| {
                rule_error(&format!("predicate kind {kind:?} needs a number {field:?}"))
            })
        };
        let value = |field: &'static str| -> Result<Value, DomainError> {
            map.get(field).cloned().ok_or_else(|| {
                rule_error(&format!("predicate kind {kind:?} needs a {field:?}"))
            })
        };

        Ok(match kind {
            "exists" => Predicate::Exists {
                variable: variable("variable")?,
            },
            "missing" => Predicate::Missing {
                variable: variable("variable")?,
            },
            "equals" => Predicate::Equals {
                variable: variable("variable")?,
                value: value("value")?,
            },
            "not_equals" => Predicate::NotEquals {
                variable: variable("variable")?,
                value: value("value")?,
            },
            "number_at_least" => Predicate::NumberAtLeast {
                variable: variable("variable")?,
                minimum: number("minimum")?,
            },
            "number_below" => Predicate::NumberBelow {
                variable: variable("variable")?,
                maximum: number("maximum")?,
            },
            "string_before" => Predicate::StringBefore {
                variable: variable("variable")?,
                than: variable("than")?,
            },
            "string_after" => Predicate::StringAfter {
                variable: variable("variable")?,
                than: variable("than")?,
            },
            "contains" => Predicate::Contains {
                variable: variable("variable")?,
                value: value("value")?,
            },
            "has_key" => Predicate::HasKey {
                variable: variable("variable")?,
                key: variable("key")?,
            },
            "nonempty" => Predicate::Nonempty {
                variable: variable("variable")?,
            },
            "count_at_least" => {
                let minimum = map.get("minimum").and_then(Value::as_u64).ok_or_else(|| {
                    rule_error("predicate kind \"count_at_least\" needs a non-negative integer \"minimum\"")
                })?;
                Predicate::CountAtLeast {
                    variable: variable("variable")?,
                    minimum: minimum as usize,
                }
            }
            "all_of" | "any_of" => {
                let limbs = map
                    .get("predicates")
                    .and_then(Value::as_array)
                    .ok_or_else(|| {
                        rule_error(&format!(
                            "predicate kind {kind:?} needs an array \"predicates\""
                        ))
                    })?;
                if limbs.is_empty() {
                    return Err(rule_error(&format!(
                        "predicate kind {kind:?} with no limbs has no truth value to declare"
                    )));
                }
                let parsed = limbs
                    .iter()
                    .map(Predicate::from_json)
                    .collect::<Result<Vec<_>, _>>()?;
                if kind == "all_of" {
                    Predicate::AllOf { predicates: parsed }
                } else {
                    Predicate::AnyOf { predicates: parsed }
                }
            }
            "not" => Predicate::Not {
                predicate: Box::new(Predicate::from_json(map.get("predicate").ok_or_else(
                    || rule_error("predicate kind \"not\" needs a \"predicate\""),
                )?)?),
            },
            _ => unreachable!("kind was validated against the declared list"),
        })
    }

    /// Every variable this predicate reads, for the witness's observed bindings.
    pub fn variables(&self) -> BTreeSet<String> {
        let mut names = BTreeSet::new();
        self.collect_variables(&mut names);
        names
    }

    fn collect_variables(&self, into: &mut BTreeSet<String>) {
        match self {
            Predicate::Exists { variable }
            | Predicate::Missing { variable }
            | Predicate::Equals { variable, .. }
            | Predicate::NotEquals { variable, .. }
            | Predicate::NumberAtLeast { variable, .. }
            | Predicate::NumberBelow { variable, .. }
            | Predicate::StringBefore { variable, .. }
            | Predicate::StringAfter { variable, .. }
            | Predicate::Contains { variable, .. }
            | Predicate::HasKey { variable, .. }
            | Predicate::Nonempty { variable }
            | Predicate::CountAtLeast { variable, .. } => {
                into.insert(variable.clone());
            }
            Predicate::AllOf { predicates } | Predicate::AnyOf { predicates } => {
                for predicate in predicates {
                    predicate.collect_variables(into);
                }
            }
            Predicate::Not { predicate } => predicate.collect_variables(into),
        }
    }

    /// Strong three-valued evaluation. `Err` is "unevaluable", not "false".
    pub fn evaluate(&self, values: &BTreeMap<String, Value>) -> Result<bool, Obstruction> {
        match self {
            Predicate::Exists { variable } => Ok(values.contains_key(variable)),
            Predicate::Missing { variable } => Ok(!values.contains_key(variable)),
            Predicate::Equals { variable, value } => {
                Ok(required(values, variable)? == value)
            }
            Predicate::NotEquals { variable, value } => {
                Ok(required(values, variable)? != value)
            }
            Predicate::NumberAtLeast { variable, minimum } => {
                Ok(number(values, variable)? >= *minimum)
            }
            Predicate::NumberBelow { variable, maximum } => {
                Ok(number(values, variable)? < *maximum)
            }
            Predicate::StringBefore { variable, than } => {
                Ok(string(values, variable)? < than.as_str())
            }
            Predicate::StringAfter { variable, than } => {
                Ok(string(values, variable)? > than.as_str())
            }
            Predicate::Contains { variable, value } => {
                let found = required(values, variable)?;
                let items = found.as_array().ok_or_else(|| {
                    Obstruction::wrong_type(variable, "an array", found)
                })?;
                Ok(items.contains(value))
            }
            Predicate::HasKey { variable, key } => {
                let found = required(values, variable)?;
                let object = found.as_object().ok_or_else(|| {
                    Obstruction::wrong_type(variable, "an object", found)
                })?;
                Ok(object.contains_key(key))
            }
            Predicate::Nonempty { variable } => Ok(collection_len(values, variable)? > 0),
            Predicate::CountAtLeast { variable, minimum } => {
                Ok(collection_len(values, variable)? >= *minimum)
            }
            Predicate::AllOf { predicates } => {
                let mut obstruction = None;
                for predicate in predicates {
                    match predicate.evaluate(values) {
                        Ok(false) => return Ok(false),
                        Ok(true) => {}
                        Err(blocked) => {
                            obstruction.get_or_insert(blocked);
                        }
                    };
                }
                match obstruction {
                    Some(blocked) => Err(blocked),
                    None => Ok(true),
                }
            }
            Predicate::AnyOf { predicates } => {
                let mut obstruction = None;
                for predicate in predicates {
                    match predicate.evaluate(values) {
                        Ok(true) => return Ok(true),
                        Ok(false) => {}
                        Err(blocked) => {
                            obstruction.get_or_insert(blocked);
                        }
                    };
                }
                match obstruction {
                    Some(blocked) => Err(blocked),
                    None => Ok(false),
                }
            }
            Predicate::Not { predicate } => Ok(!predicate.evaluate(values)?),
        }
    }
}

fn required<'a>(
    values: &'a BTreeMap<String, Value>,
    variable: &str,
) -> Result<&'a Value, Obstruction> {
    values.get(variable).ok_or_else(|| Obstruction::absent(variable))
}

fn number(values: &BTreeMap<String, Value>, variable: &str) -> Result<f64, Obstruction> {
    let found = required(values, variable)?;
    found
        .as_f64()
        .ok_or_else(|| Obstruction::wrong_type(variable, "a number", found))
}

fn string<'a>(
    values: &'a BTreeMap<String, Value>,
    variable: &str,
) -> Result<&'a str, Obstruction> {
    let found = required(values, variable)?;
    found
        .as_str()
        .ok_or_else(|| Obstruction::wrong_type(variable, "a string", found))
}

fn collection_len(values: &BTreeMap<String, Value>, variable: &str) -> Result<usize, Obstruction> {
    let found = required(values, variable)?;
    match found {
        Value::Array(items) => Ok(items.len()),
        Value::Object(entries) => Ok(entries.len()),
        other => Err(Obstruction::wrong_type(variable, "an array or object", other)),
    }
}

/// One named violation detector.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleCheck {
    pub name: String,
    pub description: String,
    pub when: Predicate,
}

impl RuleCheck {
    fn from_json(document: &Value) -> Result<RuleCheck, DomainError> {
        let map = strict_object(document, "check", &["name", "description", "when"])?;
        Ok(RuleCheck {
            name: required_string(map, "check", "name")?,
            description: required_string(map, "check", "description")?,
            when: Predicate::from_json(map.get("when").ok_or_else(|| {
                rule_error("check declares no \"when\" predicate")
            })?)?,
        })
    }

    /// The fired-check witness: the rule's name, the bindings it read (canonically rendered so
    /// a human can re-run the check by hand), and the declared sentence.
    fn violation_witness(&self, values: &BTreeMap<String, Value>) -> LeakageWitness {
        LeakageWitness::DomainCheck {
            check: self.name.clone(),
            observed: self.observed_bindings(values),
            detail: self.description.clone(),
        }
    }

    /// The unrun-check witness. The detail states that the check did not run and why, so an
    /// `underdetermined` verdict is as checkable as an `invalid` one.
    fn obstruction_witness(
        &self,
        values: &BTreeMap<String, Value>,
        obstruction: &Obstruction,
    ) -> LeakageWitness {
        LeakageWitness::DomainCheck {
            check: self.name.clone(),
            observed: self.observed_bindings(values),
            detail: format!(
                "check did not run: variable {:?} {}",
                obstruction.variable, obstruction.reason
            ),
        }
    }

    fn observed_bindings(&self, values: &BTreeMap<String, Value>) -> BTreeMap<String, String> {
        self.when
            .variables()
            .into_iter()
            .map(|variable| {
                let rendered = match values.get(&variable) {
                    Some(value) => to_canonical_string(value)
                        .unwrap_or_else(|_| "unrenderable".to_string()),
                    None => "absent".to_string(),
                };
                (variable, rendered)
            })
            .collect()
    }
}

/// A deterministic oracle declared as data. See the module docs for the verdict semantics.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleOracle {
    kind: String,
    require: Vec<String>,
    checks: Vec<RuleCheck>,
}

/// The check name used on the witnesses of an up-front `require` abstention.
pub const REQUIRED_EVIDENCE_CHECK: &str = "required_evidence";

impl RuleOracle {
    /// Parses `{"kind": "rule/...", "require": [...], "checks": [...]}` strictly.
    ///
    /// The kind must begin with `"rule/"` so any consumer of a certificate can tell a declared
    /// rule oracle from a native one by its verdict alone.
    pub fn from_json(document: &Value) -> Result<RuleOracle, DomainError> {
        let map = strict_object(document, "rule oracle", &["kind", "require", "checks"])?;

        let kind = required_string(map, "rule oracle", "kind")?;
        if !kind.starts_with("rule/") {
            return Err(rule_error(&format!(
                "oracle kind {kind:?} must begin with \"rule/\" so a verdict names its origin"
            )));
        }

        let require = match map.get("require") {
            None => Vec::new(),
            Some(list) => list
                .as_array()
                .ok_or_else(|| rule_error("\"require\" is not an array"))?
                .iter()
                .map(|item| {
                    item.as_str()
                        .map(str::to_string)
                        .ok_or_else(|| rule_error("\"require\" carries a non-string entry"))
                })
                .collect::<Result<Vec<_>, _>>()?,
        };

        let checks = map
            .get("checks")
            .and_then(Value::as_array)
            .ok_or_else(|| rule_error("rule oracle declares no \"checks\" array"))?
            .iter()
            .map(RuleCheck::from_json)
            .collect::<Result<Vec<_>, _>>()?;
        if checks.is_empty() {
            return Err(rule_error(
                "a rule oracle with no checks would return valid for every world; declare at least one check",
            ));
        }
        let mut seen = BTreeSet::new();
        for check in &checks {
            if !seen.insert(check.name.as_str()) {
                return Err(rule_error(&format!(
                    "duplicate check name {:?}; witnesses must name their rule unambiguously",
                    check.name
                )));
            }
        }

        Ok(RuleOracle {
            kind,
            require,
            checks,
        })
    }

    pub fn checks(&self) -> &[RuleCheck] {
        &self.checks
    }

    pub fn required_variables(&self) -> &[String] {
        &self.require
    }
}

impl DecisionOracle for RuleOracle {
    fn kind(&self) -> &str {
        &self.kind
    }

    fn evaluate(&self, values: &BTreeMap<String, Value>) -> Result<OracleVerdict, FiberError> {
        let missing: Vec<&String> = self
            .require
            .iter()
            .filter(|variable| !values.contains_key(variable.as_str()))
            .collect();
        if !missing.is_empty() {
            let witnesses = missing
                .into_iter()
                .map(|variable| LeakageWitness::DomainCheck {
                    check: REQUIRED_EVIDENCE_CHECK.into(),
                    observed: BTreeMap::from([(variable.clone(), "absent".to_string())]),
                    detail: format!(
                        "required variable {variable:?} was not delivered by the compiled region"
                    ),
                })
                .collect();
            return Ok(OracleVerdict::abstain(&self.kind, witnesses));
        }

        let mut violations = Vec::new();
        let mut unrun = Vec::new();
        for check in &self.checks {
            match check.when.evaluate(values) {
                Ok(true) => violations.push(check.violation_witness(values)),
                Ok(false) => {}
                Err(obstruction) => {
                    unrun.push(check.obstruction_witness(values, &obstruction))
                }
            }
        }

        if violations.is_empty() {
            if unrun.is_empty() {
                Ok(OracleVerdict::new(&self.kind, Vec::new()))
            } else {
                Ok(OracleVerdict::abstain(&self.kind, unrun))
            }
        } else {
            violations.extend(unrun);
            Ok(OracleVerdict::new(&self.kind, violations))
        }
    }
}

fn strict_object<'a>(
    document: &'a Value,
    what: &str,
    declared: &[&str],
) -> Result<&'a Map<String, Value>, DomainError> {
    let map = document
        .as_object()
        .ok_or_else(|| rule_error(&format!("{what} is not an object")))?;
    if let Some(unknown) = map.keys().find(|key| !declared.contains(&key.as_str())) {
        return Err(rule_error(&format!(
            "undeclared field {unknown:?} on {what}"
        )));
    }
    Ok(map)
}

fn required_string(
    map: &Map<String, Value>,
    what: &str,
    field: &str,
) -> Result<String, DomainError> {
    map.get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| rule_error(&format!("{what} needs a string {field:?}")))
}

fn rule_error(message: &str) -> DomainError {
    DomainError::Rules(message.to_string())
}
