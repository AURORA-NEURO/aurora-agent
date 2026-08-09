//! A property-based oracle (31.03).
//!
//! 31.03 asks for tests of "shape, monotonicity, conservation, and invariance properties" — checks
//! that go beyond byte equality without needing a semantic reviewer. The four properties here are
//! the ones expressible over a JSON artifact without domain readers:
//! ordering of instants, numeric bounds, monotonicity of a series, and conservation of a total.
//!
//! # Why this sits below `execution` on the ladder
//!
//! A property oracle is fully reproducible — [`crate::Determinism::Reproducible`], same as a
//! schema oracle — yet it occupies a weaker rung. The reason is what a *pass* means. A satisfied
//! checksum says the bytes are the bytes. A satisfied property says only that the properties
//! somebody thought to write down were not violated, and 31.02's own metrics list "property
//! coverage" as a thing that can be low. Failures from this oracle are as hard as any; passes are
//! weaker, and the ladder ranks by what agreement is worth.
//!
//! # Ordering is on parsed instants, not strings
//!
//! [`NumericProperty::OrderedInstants`] parses both operands as [`UtcTimestamp`] and returns
//! [`OracleError::NonComparableField`] if either fails. `crates/fiber/src/oracle.rs` compares
//! timestamps lexicographically and documents that as bug-compatibility with a CPython reference;
//! this crate is not bound by that reference, so a timestamp that cannot be ordered is a
//! configuration error rather than a silently wrong comparison.

use serde_json::Value;

use crate::error::OracleError;
use crate::evidence::Evidence;
use crate::judgement::{Confidence, Finding, Judgement, Position};
use crate::ladder::EvidenceTier;
use crate::manifest::{OracleId, OracleManifest, OracleRef, OracleVersion};
use crate::oracle::Oracle;
use crate::plane::Plane;
use crate::time::{UtcTimestamp, ValidityWindow};

/// A named scientific property over the artifact.
#[derive(Debug, Clone, PartialEq)]
pub enum NumericProperty {
    /// One instant must not follow another. 31.02's worked case — an event date preceding the
    /// index date — is this property inverted.
    OrderedInstants { earlier: String, later: String },
    /// A number must lie inside a closed interval.
    Bounded { field: String, low: f64, high: f64 },
    /// An array of numbers must never decrease.
    NonDecreasing { series: String },
    /// Parts must sum to a declared total within a tolerance.
    ConservesTotal {
        parts: Vec<String>,
        total: String,
        tolerance: f64,
    },
}

impl NumericProperty {
    /// The name that appears in [`Finding::PropertyViolated`] and in skip records.
    pub fn name(&self) -> String {
        match self {
            NumericProperty::OrderedInstants { earlier, later } => {
                format!("ordered_instants({earlier} <= {later})")
            }
            NumericProperty::Bounded { field, low, high } => {
                format!("bounded({field} in [{low}, {high}])")
            }
            NumericProperty::NonDecreasing { series } => format!("non_decreasing({series})"),
            NumericProperty::ConservesTotal { total, .. } => format!("conserves_total({total})"),
        }
    }

    fn pointer(&self) -> String {
        match self {
            NumericProperty::OrderedInstants { later, .. } => later.clone(),
            NumericProperty::Bounded { field, .. } => field.clone(),
            NumericProperty::NonDecreasing { series } => series.clone(),
            NumericProperty::ConservesTotal { total, .. } => total.clone(),
        }
    }
}

/// Evaluates a list of [`NumericProperty`] over an artifact.
pub struct PropertyOracle {
    manifest: OracleManifest,
    properties: Vec<NumericProperty>,
}

impl PropertyOracle {
    /// Builds a property oracle at [`EvidenceTier::Property`], establishing only
    /// [`Plane::Analytical`].
    pub fn new(
        id: impl Into<String>,
        version: OracleVersion,
        validity: ValidityWindow,
    ) -> Result<Self, OracleError> {
        let manifest = OracleManifest::new(
            OracleRef::new(OracleId::parse(id)?, version),
            EvidenceTier::Property,
            [Plane::Analytical],
            [],
            validity,
        )?
        .disclaiming_the_rest()
        .with_failure_mode(
            "a pass means only that the configured properties held; property coverage is not \
             measured here and an unwritten property cannot fail",
        );

        Ok(PropertyOracle {
            manifest,
            properties: Vec::new(),
        })
    }

    pub fn check(mut self, property: NumericProperty) -> Self {
        self.properties.push(property);
        self
    }

    pub fn manifest_mut(&mut self) -> &mut OracleManifest {
        &mut self.manifest
    }

    fn evaluate_one(
        &self,
        evidence: &Evidence,
        property: &NumericProperty,
    ) -> Result<Option<Finding>, OracleError> {
        match property {
            NumericProperty::OrderedInstants { earlier, later } => {
                let (Some(first), Some(second)) = (evidence.field(earlier), evidence.field(later))
                else {
                    return Ok(Some(skipped(property, "one of the instants is absent")));
                };
                let first = self.instant(earlier, first)?;
                let second = self.instant(later, second)?;
                if first <= second {
                    Ok(None)
                } else {
                    Ok(Some(Finding::PropertyViolated {
                        property: property.name(),
                        pointer: property.pointer(),
                        detail: format!("{first} follows {second}"),
                    }))
                }
            }
            NumericProperty::Bounded { field, low, high } => {
                let Some(value) = evidence.number(field) else {
                    return Ok(Some(skipped(
                        property,
                        "the field is absent or not a number",
                    )));
                };
                if value >= *low && value <= *high {
                    Ok(None)
                } else {
                    Ok(Some(Finding::PropertyViolated {
                        property: property.name(),
                        pointer: field.clone(),
                        detail: format!("{value} lies outside [{low}, {high}]"),
                    }))
                }
            }
            NumericProperty::NonDecreasing { series } => {
                let Some(values) = evidence.field(series).and_then(Value::as_array) else {
                    return Ok(Some(skipped(
                        property,
                        "the series is absent or not an array",
                    )));
                };
                let mut numbers = Vec::with_capacity(values.len());
                for value in values {
                    let Some(number) = value.as_f64() else {
                        return Err(OracleError::NonComparableField {
                            kind: self.manifest.kind().to_string(),
                            pointer: series.clone(),
                            expected: "an array of numbers",
                            actual: format!("an element of type {}", type_of(value)),
                        });
                    };
                    numbers.push(number);
                }
                match numbers.windows(2).position(|pair| pair[1] < pair[0]) {
                    None => Ok(None),
                    Some(index) => Ok(Some(Finding::PropertyViolated {
                        property: property.name(),
                        pointer: format!("{series}[{}]", index + 1),
                        detail: format!(
                            "element {} is {} but element {} is {}",
                            index,
                            numbers[index],
                            index + 1,
                            numbers[index + 1]
                        ),
                    })),
                }
            }
            NumericProperty::ConservesTotal {
                parts,
                total,
                tolerance,
            } => {
                let Some(declared) = evidence.number(total) else {
                    return Ok(Some(skipped(
                        property,
                        "the total is absent or not a number",
                    )));
                };
                let mut sum = 0.0;
                for part in parts {
                    let Some(value) = evidence.number(part) else {
                        return Ok(Some(skipped(
                            property,
                            "at least one part is absent or not a number",
                        )));
                    };
                    sum += value;
                }
                if (sum - declared).abs() <= *tolerance {
                    Ok(None)
                } else {
                    Ok(Some(Finding::PropertyViolated {
                        property: property.name(),
                        pointer: total.clone(),
                        detail: format!(
                            "parts sum to {sum} but the declared total is {declared} \
                             (tolerance {tolerance})"
                        ),
                    }))
                }
            }
        }
    }

    fn instant(&self, pointer: &str, value: &Value) -> Result<UtcTimestamp, OracleError> {
        let text = value
            .as_str()
            .ok_or_else(|| OracleError::NonComparableField {
                kind: self.manifest.kind().to_string(),
                pointer: pointer.to_string(),
                expected: "a UTC timestamp string",
                actual: type_of(value).to_string(),
            })?;
        UtcTimestamp::parse(text).map_err(|_| OracleError::NonComparableField {
            kind: self.manifest.kind().to_string(),
            pointer: pointer.to_string(),
            expected: "a UTC timestamp of the form YYYY-MM-DDTHH:MM:SSZ",
            actual: text.to_string(),
        })
    }
}

impl Oracle for PropertyOracle {
    fn manifest(&self) -> &OracleManifest {
        &self.manifest
    }

    /// An oracle configured with no properties, or whose every property was skipped for want of
    /// inputs, abstains. Reporting `Supported` there would be a pass earned by doing no work, and
    /// it is the most common way a property suite quietly stops checking anything.
    fn evaluate(&self, evidence: &Evidence) -> Result<Judgement, OracleError> {
        let mut findings = Vec::new();
        for property in &self.properties {
            if let Some(finding) = self.evaluate_one(evidence, property)? {
                findings.push(finding);
            }
        }

        let violated = findings.iter().any(Finding::is_violation);
        let skipped_all = !violated && findings.len() == self.properties.len();

        let position = if violated {
            Position::Contradicted
        } else if skipped_all {
            Position::NotEvaluable
        } else {
            Position::Supported
        };

        Ok(
            Judgement::from_manifest(&self.manifest, &evidence.at, position, Confidence::CERTAIN)
                .with_findings(findings)
                .with_rationale(format!(
                    "evaluated {} configured propert(ies)",
                    self.properties.len()
                )),
        )
    }
}

/// Records a check that could not run. Kept as a finding rather than dropped, because a property
/// suite that silently skips everything reports the same `Supported` as one that checked
/// everything.
fn skipped(property: &NumericProperty, reason: &str) -> Finding {
    Finding::NotApplicable {
        check: property.name(),
        reason: reason.to_string(),
    }
}

fn type_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
