//! Data-quality gates as executable predicates over a dataset.
//!
//! Blueprint 12.11 asks for "evaluation telemetry and provenance signals" and 40.31 for quality
//! gates in the test pyramid. Both are usually implemented as a score: a percentage of rows
//! passing, a traffic light, a number that goes in a dashboard. A score is the wrong shape,
//! because it silently merges two things that must not merge.
//!
//! # Three outcomes, not two
//!
//! A check over a dataset can end three ways:
//!
//! - [`CheckOutcome::Pass`], with the number of values actually examined;
//! - [`CheckOutcome::Fail`], with a [`Witness`] — a row, a column, the offending value and what
//!   was expected. Concrete and checkable, in the sense `bioprism-lens` and `bioprism-section`
//!   use the word: not "97% conformant" but "row 4 of `stage` is `IV+`, which is not one of the
//!   allowed values";
//! - [`CheckOutcome::NotRunnable`], with a [`NotRunnable`] reason — the column is absent, every
//!   value is null, the values are not numbers, the reference set was not supplied.
//!
//! The third is the one that gets collapsed. A gate that treats a missing column as a pass ships
//! a pipeline whose most important check has silently never run; a gate that treats it as a fail
//! trains its operators to ignore red. Neither is true, and this module refuses to pick one:
//! [`GateVerdict::Indeterminate`] is a distinct verdict, and [`GateVerdict::Passed`] requires
//! that *no* check was unrunnable.
//!
//! # Deliberately not implemented
//!
//! No statistical checks (distributions, drift, outliers) — those return a score, and a score
//! needs a threshold nobody in the blueprint sets. No cross-dataset joins beyond the single
//! [`ReferenceSets`] lookup. No sampling: every check reads every row, because a sampled check
//! that passes is a fourth outcome this module has no room for. No schema inference, no repair,
//! no quarantine of failing rows. The dataset is a column map held in memory; there is no reader,
//! no Parquet, no SQL, and no connection to `bioprism-store`, which indexes worlds rather than
//! tables.

use crate::error::QualityError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// A columnar dataset held in memory.
///
/// Columns are `serde_json::Value` so a check can distinguish null from absent from
/// wrong-typed — a `Vec<f64>` would have had to invent a sentinel for null and the sentinel
/// would eventually be mistaken for data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dataset {
    name: String,
    columns: BTreeMap<String, Vec<Value>>,
    rows: usize,
}

impl Dataset {
    pub fn new(name: impl Into<String>) -> Result<Self, QualityError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(QualityError::MalformedField {
                field: "dataset name",
                value: name,
            });
        }
        Ok(Dataset {
            name,
            columns: BTreeMap::new(),
            rows: 0,
        })
    }

    /// Adds a column. The first column fixes the row count; a later column of a different length
    /// is refused, because a ragged dataset makes every row-indexed witness ambiguous.
    pub fn with_column(
        mut self,
        column: impl Into<String>,
        values: impl IntoIterator<Item = Value>,
    ) -> Result<Self, QualityError> {
        let column = column.into();
        let values: Vec<Value> = values.into_iter().collect();
        if self.columns.contains_key(&column) {
            return Err(QualityError::DuplicateColumn(column));
        }
        if self.columns.is_empty() {
            self.rows = values.len();
        } else if values.len() != self.rows {
            return Err(QualityError::RaggedColumn {
                column,
                found: values.len(),
                expected: self.rows,
            });
        }
        self.columns.insert(column, values);
        Ok(self)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn column(&self, name: &str) -> Option<&[Value]> {
        self.columns.get(name).map(Vec::as_slice)
    }

    pub fn column_names(&self) -> BTreeSet<&str> {
        self.columns.keys().map(String::as_str).collect()
    }
}

/// Named sets a foreign-key check can be resolved against.
///
/// Kept separate from the dataset because the absence of a reference set is a property of the
/// *run*, not of the data, and it must produce [`NotRunnable`] rather than a failure.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceSets {
    sets: BTreeMap<String, BTreeSet<String>>,
}

impl ReferenceSets {
    pub fn new() -> Self {
        ReferenceSets::default()
    }

    pub fn with(
        mut self,
        name: impl Into<String>,
        members: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.sets
            .insert(name.into(), members.into_iter().map(Into::into).collect());
        self
    }

    pub fn get(&self, name: &str) -> Option<&BTreeSet<String>> {
        self.sets.get(name)
    }
}

/// A concrete failing observation.
///
/// Every field is here so a reader can go and look at the data and see the same thing. A witness
/// that said only "3 rows failed" would be a score with extra steps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Witness {
    pub row: usize,
    pub column: String,
    pub found: String,
    pub expected: String,
}

/// Why a check could not run.
///
/// These are not failures and must never be reported as failures. Each names something about the
/// run rather than about the data's conformance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotRunnable {
    /// The dataset has no such column, so the check has nothing to examine.
    MissingColumn { column: String },
    /// Every value in the column is null. A range or ordering check over nothing is vacuous, and
    /// reporting a vacuous pass is how an empty ingest gets promoted to production.
    AllValuesNull { column: String },
    /// A value is present and is not of the type the check needs. Distinct from a failure: the
    /// check is about range or ordering, and a string in a numeric column is a schema problem a
    /// different check should be raising.
    NotComparable {
        column: String,
        row: usize,
        found: String,
    },
    /// A foreign-key check named a reference set that was not supplied to this run.
    MissingReferenceSet { reference: String },
}

impl NotRunnable {
    pub fn name(&self) -> &'static str {
        match self {
            NotRunnable::MissingColumn { .. } => "missing-column",
            NotRunnable::AllValuesNull { .. } => "all-values-null",
            NotRunnable::NotComparable { .. } => "not-comparable",
            NotRunnable::MissingReferenceSet { .. } => "missing-reference-set",
        }
    }
}

/// What one check concluded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckOutcome {
    /// Held, over `examined` values. The count is reported because a pass over zero values and a
    /// pass over ten thousand are not the same evidence.
    Pass { examined: usize },
    /// Did not hold, and here is where.
    Fail { witness: Witness },
    /// Could not be evaluated.
    NotRunnable { reason: NotRunnable },
}

impl CheckOutcome {
    pub fn is_pass(&self) -> bool {
        matches!(self, CheckOutcome::Pass { .. })
    }

    pub fn is_fail(&self) -> bool {
        matches!(self, CheckOutcome::Fail { .. })
    }

    pub fn is_not_runnable(&self) -> bool {
        matches!(self, CheckOutcome::NotRunnable { .. })
    }

    pub fn witness(&self) -> Option<&Witness> {
        match self {
            CheckOutcome::Fail { witness } => Some(witness),
            _ => None,
        }
    }
}

/// An executable predicate over a dataset.
///
/// Data-carrying rather than a boxed closure, so a gate can be serialized, transported, and read
/// by a human deciding whether the gate says what they meant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Check {
    /// No value in the column is null.
    NotNull { column: String },
    /// No two non-null values in the column are equal.
    Unique { column: String },
    /// Every non-null numeric value lies within the closed interval.
    InRange { column: String, min: f64, max: f64 },
    /// Every non-null value, rendered as a string, is one of the allowed values.
    OneOf {
        column: String,
        allowed: BTreeSet<String>,
    },
    /// The dataset has at least this many rows.
    RowCountAtLeast { rows: usize },
    /// Non-null numeric values do not decrease from one row to the next.
    NonDecreasing { column: String },
    /// Every non-null value appears in the named reference set.
    ForeignKey { column: String, reference: String },
}

fn render(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

impl Check {
    /// Evaluates the check.
    ///
    /// Null handling is uniform and stated once: **nulls are skipped by every check except
    /// [`Check::NotNull`]**, and a column whose values are all null is
    /// [`NotRunnable::AllValuesNull`] rather than a vacuous pass. That rule is a decision, not a
    /// blueprint requirement — 12.11 does not address null semantics — and it is the one that
    /// keeps "we checked and it held" apart from "there was nothing to check".
    pub fn run(&self, data: &Dataset, references: &ReferenceSets) -> CheckOutcome {
        match self {
            Check::RowCountAtLeast { rows } => {
                if data.rows() >= *rows {
                    CheckOutcome::Pass { examined: 1 }
                } else {
                    CheckOutcome::Fail {
                        witness: Witness {
                            row: 0,
                            column: "*".to_string(),
                            found: data.rows().to_string(),
                            expected: format!("at least {rows} rows"),
                        },
                    }
                }
            }
            Check::NotNull { column } => self.over(data, column, |values| {
                for (row, value) in values.iter().enumerate() {
                    if value.is_null() {
                        return Some(CheckOutcome::Fail {
                            witness: Witness {
                                row,
                                column: column.clone(),
                                found: "null".to_string(),
                                expected: "a value".to_string(),
                            },
                        });
                    }
                }
                None
            }),
            Check::Unique { column } => self.over(data, column, |values| {
                let mut seen: BTreeMap<String, usize> = BTreeMap::new();
                for (row, value) in values.iter().enumerate() {
                    if value.is_null() {
                        continue;
                    }
                    let rendered = render(value);
                    if let Some(first) = seen.get(&rendered) {
                        return Some(CheckOutcome::Fail {
                            witness: Witness {
                                row,
                                column: column.clone(),
                                found: rendered,
                                expected: format!("a value not already seen at row {first}"),
                            },
                        });
                    }
                    seen.insert(rendered, row);
                }
                None
            }),
            Check::OneOf { column, allowed } => self.over(data, column, |values| {
                for (row, value) in values.iter().enumerate() {
                    if value.is_null() {
                        continue;
                    }
                    let rendered = render(value);
                    if !allowed.contains(&rendered) {
                        return Some(CheckOutcome::Fail {
                            witness: Witness {
                                row,
                                column: column.clone(),
                                found: rendered,
                                expected: format!(
                                    "one of {}",
                                    allowed.iter().cloned().collect::<Vec<_>>().join(", ")
                                ),
                            },
                        });
                    }
                }
                None
            }),
            Check::InRange { column, min, max } => self.over(data, column, |values| {
                for (row, value) in values.iter().enumerate() {
                    if value.is_null() {
                        continue;
                    }
                    let Some(number) = value.as_f64() else {
                        return Some(CheckOutcome::NotRunnable {
                            reason: NotRunnable::NotComparable {
                                column: column.clone(),
                                row,
                                found: render(value),
                            },
                        });
                    };
                    if number < *min || number > *max {
                        return Some(CheckOutcome::Fail {
                            witness: Witness {
                                row,
                                column: column.clone(),
                                found: render(value),
                                expected: format!("a value in [{min}, {max}]"),
                            },
                        });
                    }
                }
                None
            }),
            Check::NonDecreasing { column } => self.over(data, column, |values| {
                let mut previous: Option<(usize, f64)> = None;
                for (row, value) in values.iter().enumerate() {
                    if value.is_null() {
                        continue;
                    }
                    let Some(number) = value.as_f64() else {
                        return Some(CheckOutcome::NotRunnable {
                            reason: NotRunnable::NotComparable {
                                column: column.clone(),
                                row,
                                found: render(value),
                            },
                        });
                    };
                    if let Some((earlier_row, earlier)) = previous {
                        if number < earlier {
                            return Some(CheckOutcome::Fail {
                                witness: Witness {
                                    row,
                                    column: column.clone(),
                                    found: render(value),
                                    expected: format!(
                                        "a value at least {earlier}, seen at row {earlier_row}"
                                    ),
                                },
                            });
                        }
                    }
                    previous = Some((row, number));
                }
                None
            }),
            Check::ForeignKey { column, reference } => {
                let Some(members) = references.get(reference) else {
                    return CheckOutcome::NotRunnable {
                        reason: NotRunnable::MissingReferenceSet {
                            reference: reference.clone(),
                        },
                    };
                };
                self.over(data, column, |values| {
                    for (row, value) in values.iter().enumerate() {
                        if value.is_null() {
                            continue;
                        }
                        let rendered = render(value);
                        if !members.contains(&rendered) {
                            return Some(CheckOutcome::Fail {
                                witness: Witness {
                                    row,
                                    column: column.clone(),
                                    found: rendered,
                                    expected: format!("a member of {reference}"),
                                },
                            });
                        }
                    }
                    None
                })
            }
        }
    }

    /// Resolves the column, applies the shared null rule, and runs `body` over the values.
    fn over(
        &self,
        data: &Dataset,
        column: &str,
        body: impl FnOnce(&[Value]) -> Option<CheckOutcome>,
    ) -> CheckOutcome {
        let Some(values) = data.column(column) else {
            return CheckOutcome::NotRunnable {
                reason: NotRunnable::MissingColumn {
                    column: column.to_string(),
                },
            };
        };
        let non_null = values.iter().filter(|value| !value.is_null()).count();
        let counts_nulls = matches!(self, Check::NotNull { .. });
        if !counts_nulls && non_null == 0 && !values.is_empty() {
            return CheckOutcome::NotRunnable {
                reason: NotRunnable::AllValuesNull {
                    column: column.to_string(),
                },
            };
        }
        match body(values) {
            Some(outcome) => outcome,
            None => CheckOutcome::Pass {
                examined: if counts_nulls { values.len() } else { non_null },
            },
        }
    }
}

/// A named set of checks run together.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gate {
    name: String,
    checks: BTreeMap<String, Check>,
}

impl Gate {
    pub fn new(name: impl Into<String>) -> Result<Self, QualityError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(QualityError::MalformedField {
                field: "gate name",
                value: name,
            });
        }
        Ok(Gate {
            name,
            checks: BTreeMap::new(),
        })
    }

    /// Adds a named check. A repeated name is refused rather than overwriting, because a silently
    /// dropped check makes the gate pass for a reason nobody can reconstruct.
    pub fn with(mut self, name: impl Into<String>, check: Check) -> Result<Self, QualityError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(QualityError::MalformedField {
                field: "check name",
                value: name,
            });
        }
        if self.checks.contains_key(&name) {
            return Err(QualityError::DuplicateCheckName(name));
        }
        self.checks.insert(name, check);
        Ok(self)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn len(&self) -> usize {
        self.checks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.checks.is_empty()
    }

    /// Runs every check and reaches a verdict.
    pub fn run(&self, data: &Dataset, references: &ReferenceSets) -> GateReport {
        let outcomes: BTreeMap<String, CheckOutcome> = self
            .checks
            .iter()
            .map(|(name, check)| (name.clone(), check.run(data, references)))
            .collect();

        let failing: BTreeSet<String> = outcomes
            .iter()
            .filter(|(_, outcome)| outcome.is_fail())
            .map(|(name, _)| name.clone())
            .collect();
        let not_runnable: BTreeSet<String> = outcomes
            .iter()
            .filter(|(_, outcome)| outcome.is_not_runnable())
            .map(|(name, _)| name.clone())
            .collect();

        let verdict = if !failing.is_empty() {
            GateVerdict::Failed {
                failing,
                not_runnable,
            }
        } else if !not_runnable.is_empty() {
            GateVerdict::Indeterminate { not_runnable }
        } else {
            GateVerdict::Passed {
                checks: outcomes.len(),
            }
        };

        GateReport {
            gate: self.name.clone(),
            dataset: data.name().to_string(),
            rows: data.rows(),
            outcomes,
            verdict,
        }
    }
}

/// What a gate concluded, and why.
///
/// Three verdicts because there are three kinds of outcome, and because the two non-passing ones
/// call for different action: a failure means fix the data, an indeterminate means fix the run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GateVerdict {
    /// Every check ran and every check held.
    Passed { checks: usize },
    /// At least one check failed. Any checks that could not run are carried alongside, because a
    /// failure does not excuse an unrun check — both need attention and the report must not hide
    /// one behind the other.
    Failed {
        failing: BTreeSet<String>,
        not_runnable: BTreeSet<String>,
    },
    /// Nothing failed, but not everything ran. This is not a pass.
    Indeterminate { not_runnable: BTreeSet<String> },
}

impl GateVerdict {
    /// True only for [`GateVerdict::Passed`].
    ///
    /// Named `is_passed` rather than `is_ok` so a caller writing `if verdict.is_ok()` cannot
    /// accidentally include the indeterminate case, which is the whole mistake this type exists
    /// to prevent.
    pub fn is_passed(&self) -> bool {
        matches!(self, GateVerdict::Passed { .. })
    }

    pub fn name(&self) -> &'static str {
        match self {
            GateVerdict::Passed { .. } => "passed",
            GateVerdict::Failed { .. } => "failed",
            GateVerdict::Indeterminate { .. } => "indeterminate",
        }
    }
}

/// The outcome of every check, plus the verdict they compose to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateReport {
    pub gate: String,
    pub dataset: String,
    pub rows: usize,
    pub outcomes: BTreeMap<String, CheckOutcome>,
    pub verdict: GateVerdict,
}

impl GateReport {
    pub fn witnesses(&self) -> Vec<&Witness> {
        self.outcomes
            .values()
            .filter_map(CheckOutcome::witness)
            .collect()
    }

    /// The reasons checks could not run, by check name.
    pub fn blocked(&self) -> BTreeMap<&str, &NotRunnable> {
        self.outcomes
            .iter()
            .filter_map(|(name, outcome)| match outcome {
                CheckOutcome::NotRunnable { reason } => Some((name.as_str(), reason)),
                _ => None,
            })
            .collect()
    }
}
