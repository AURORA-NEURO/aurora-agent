//! Units, scales, normalization, and thresholds (32.16).
//!
//! 32.16's worked relation: "Drug potency is expressed in nanomolar while exposure is in micrograms
//! per milliliter; the system must convert using **molecular weight** and distinguish total from
//! unbound exposure."
//!
//! This crate does not know any molecular weight and will not pretend to. [`ConversionTable`] is
//! caller-supplied and [`convert`] fails with [`OracleXError::NoConversionFactor`] when the caller has
//! not supplied the factor for the pair being converted. Refusing is the correct behaviour: 32.16's
//! first failure risk is "silent thousand-fold errors", and every silent thousand-fold error was a
//! conversion somebody's code performed without being told how.
//!
//! # Three ways two numbers can be incomparable
//!
//! * **Dimension.** A concentration is not a mass. [`compare`] returns
//!   [`OracleXError::DimensionMismatch`].
//! * **Scale.** A log value and a linear value are different encodings of the same dimension, and
//!   subtracting one from the other produces a number with no meaning. [`Scale`] is part of
//!   [`Quantity`] and mismatches are refused.
//! * **Normalization reference.** Two values normalized against different references are two
//!   different quantities wearing one unit. [`comparable`] returns [`Determination::Unresolved`]
//!   naming the common reference as the gap — 32.16's "comparing incompatible normalized values".
//!
//! [`ExposureKind`] is the fourth, from the worked relation: total and unbound exposure share a unit
//! and are not the same quantity.
//!
//! # Thresholds
//!
//! [`threshold_call`] takes the threshold *and* the measurement's own precision, both from the
//! caller, and answers [`Determination::Unresolved`] when the value sits within precision of the
//! cut. 32.16's "threshold-neighborhood cases" are the cases where a categorical call is a coin flip
//! dressed as a measurement, and the honest output is the abstention.
//!
//! # Not implemented
//!
//! No unit registry, no dimensional algebra over products and quotients, no SI prefix parsing. A
//! [`Unit`] is a symbol plus a caller-declared dimension name. Building a real unit system means
//! shipping a table of constants, which is exactly what this crate is not allowed to invent.

use std::collections::BTreeMap;

use bioprism_oracle::EvidenceTier;
use serde::{Deserialize, Serialize};

use crate::error::OracleXError;
use crate::verdict::{Determination, Witness};

/// How a value is encoded within its dimension.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "scale", rename_all = "snake_case")]
pub enum Scale {
    Linear,
    /// The base is recorded because log2 and log10 fold-changes differ by a factor this crate must
    /// not assume.
    Log { base: String },
}

/// Whether a concentration refers to everything present or only the free fraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExposureKind {
    Total,
    Unbound,
    /// Nobody said which. Distinct from `Total`, because assuming total is how an unbound
    /// measurement gets compared against a total threshold.
    Unstated,
}

/// A symbol together with the dimension the caller says it measures.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Unit {
    pub symbol: String,
    pub dimension: String,
}

impl Unit {
    pub fn new(symbol: impl Into<String>, dimension: impl Into<String>) -> Self {
        Unit {
            symbol: symbol.into(),
            dimension: dimension.into(),
        }
    }
}

/// What a value was normalized against, when it was normalized at all.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "normalization", rename_all = "snake_case")]
pub enum Normalization {
    /// A raw measurement.
    None,
    /// Divided by, scaled to, or referenced against something named.
    Against { reference: String },
}

/// A number with everything needed to know whether it may be compared to another number.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Quantity {
    pub value: f64,
    pub unit: Unit,
    pub scale: Scale,
    pub normalization: Normalization,
    pub exposure: ExposureKind,
}

impl Quantity {
    pub fn new(value: f64, unit: Unit) -> Result<Self, OracleXError> {
        if !value.is_finite() {
            return Err(OracleXError::NonFinite {
                field: "Quantity::value",
                value,
            });
        }
        Ok(Quantity {
            value,
            unit,
            scale: Scale::Linear,
            normalization: Normalization::None,
            exposure: ExposureKind::Unstated,
        })
    }

    pub fn on_scale(mut self, scale: Scale) -> Self {
        self.scale = scale;
        self
    }

    pub fn normalized_against(mut self, reference: impl Into<String>) -> Self {
        self.normalization = Normalization::Against {
            reference: reference.into(),
        };
        self
    }

    pub fn as_exposure(mut self, exposure: ExposureKind) -> Self {
        self.exposure = exposure;
        self
    }

    pub fn describe(&self) -> String {
        format!("{} {}", self.value, self.unit.symbol)
    }
}

/// Caller-supplied conversion factors, keyed by `(from, to)` unit symbol.
///
/// Every entry is something the caller asserted and can defend. The table starts empty and this crate
/// never adds to it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ConversionTable {
    factors: BTreeMap<(String, String), f64>,
}

impl ConversionTable {
    pub fn new() -> Self {
        ConversionTable::default()
    }

    /// Declares that one `from` equals `factor` of `to`.
    ///
    /// Registers only the stated direction. The inverse is *not* inserted automatically: a caller who
    /// wants a round trip has to state both legs, and [`round_trip`] then checks that the two they
    /// stated actually agree. Deriving the inverse would make [`round_trip`] a tautology.
    pub fn declare(
        mut self,
        from: impl Into<String>,
        to: impl Into<String>,
        factor: f64,
    ) -> Result<Self, OracleXError> {
        if !factor.is_finite() || factor == 0.0 {
            return Err(OracleXError::NonFinite {
                field: "ConversionTable::factor",
                value: factor,
            });
        }
        self.factors.insert((from.into(), to.into()), factor);
        Ok(self)
    }

    pub fn factor(&self, from: &str, to: &str) -> Option<f64> {
        self.factors.get(&(from.to_string(), to.to_string())).copied()
    }
}

/// Converts a quantity into another unit, using only what the caller declared.
pub fn convert(
    quantity: &Quantity,
    to: &Unit,
    table: &ConversionTable,
) -> Result<Quantity, OracleXError> {
    if quantity.unit.dimension != to.dimension {
        return Err(OracleXError::DimensionMismatch {
            left: quantity.unit.dimension.clone(),
            right: to.dimension.clone(),
        });
    }
    if quantity.scale != Scale::Linear {
        return Err(OracleXError::DimensionMismatch {
            left: format!("{:?}", quantity.scale),
            right: "linear".to_string(),
        });
    }
    let factor = table
        .factor(&quantity.unit.symbol, &to.symbol)
        .ok_or_else(|| OracleXError::NoConversionFactor {
            from: quantity.unit.symbol.clone(),
            to: to.symbol.clone(),
        })?;
    Ok(Quantity {
        value: quantity.value * factor,
        unit: to.clone(),
        scale: quantity.scale.clone(),
        normalization: quantity.normalization.clone(),
        exposure: quantity.exposure,
    })
}

/// Whether converting out and back recovers the original within a caller-supplied tolerance.
///
/// 32.16's validation program asks for round-trip checks. The tolerance is a parameter because
/// acceptable round-trip error depends on the magnitudes involved, and a library-chosen epsilon would
/// pass everything or fail everything.
pub fn round_trip(
    quantity: &Quantity,
    via: &Unit,
    table: &ConversionTable,
    tolerance: f64,
) -> Result<Determination, OracleXError> {
    let out = convert(quantity, via, table)?;
    let back = convert(&out, &quantity.unit, table)?;
    let drift = (back.value - quantity.value).abs();
    Ok(if drift <= tolerance {
        Determination::supported(
            EvidenceTier::Deterministic,
            format!(
                "{} survives a round trip through {} within {tolerance}",
                quantity.describe(),
                via.symbol
            ),
        )
    } else {
        Determination::contradicted(
            EvidenceTier::Deterministic,
            Witness::DimensionError {
                pointer: format!("round trip {} -> {} -> {}", quantity.unit.symbol, via.symbol, quantity.unit.symbol),
                expected: quantity.describe(),
                found: back.describe(),
            },
        )
    })
}

/// Whether two quantities are the same kind of thing.
///
/// Four separate refusals, and the order they are checked in is the order a reader would want them
/// reported: dimension, then scale, then normalization reference, then exposure kind.
pub fn comparable(left: &Quantity, right: &Quantity) -> Determination {
    if left.unit.dimension != right.unit.dimension {
        return Determination::contradicted(
            EvidenceTier::Deterministic,
            Witness::DimensionError {
                pointer: format!("{} vs {}", left.describe(), right.describe()),
                expected: left.unit.dimension.clone(),
                found: right.unit.dimension.clone(),
            },
        );
    }
    if left.scale != right.scale {
        return Determination::contradicted(
            EvidenceTier::Deterministic,
            Witness::DimensionError {
                pointer: format!("{} vs {}", left.describe(), right.describe()),
                expected: format!("{:?}", left.scale),
                found: format!("{:?}", right.scale),
            },
        );
    }
    if left.normalization != right.normalization {
        return Determination::unresolved(
            "both values normalized against one reference",
            format!(
                "left is {:?} and right is {:?}; the unit is the same and the quantity is not",
                left.normalization, right.normalization
            ),
        );
    }
    if left.exposure != right.exposure {
        return Determination::unresolved(
            "a stated exposure kind for both values",
            format!(
                "left is {:?} exposure and right is {:?}; total and unbound share a unit",
                left.exposure, right.exposure
            ),
        );
    }
    Determination::supported(
        EvidenceTier::Deterministic,
        "same dimension, scale, normalization reference and exposure kind",
    )
}

/// Orders two comparable quantities, refusing incomparable ones.
pub fn compare(left: &Quantity, right: &Quantity) -> Result<std::cmp::Ordering, OracleXError> {
    if left.unit.dimension != right.unit.dimension {
        return Err(OracleXError::DimensionMismatch {
            left: left.unit.dimension.clone(),
            right: right.unit.dimension.clone(),
        });
    }
    if left.scale != right.scale || left.unit.symbol != right.unit.symbol {
        return Err(OracleXError::DimensionMismatch {
            left: format!("{} on {:?}", left.unit.symbol, left.scale),
            right: format!("{} on {:?}", right.unit.symbol, right.scale),
        });
    }
    Ok(left
        .value
        .partial_cmp(&right.value)
        .expect("both values were checked finite at construction"))
}

/// A categorical call against a cut, with an abstention band the caller's own precision defines.
///
/// `precision` is the half-width of the band, supplied by the caller because it is a property of
/// their assay. A value inside the band gets [`Determination::Unresolved`]: the measurement cannot
/// distinguish the two sides, and a call made anyway is 32.16's "threshold overfitting" with the
/// evidence for it already gone.
pub fn threshold_call(
    quantity: &Quantity,
    threshold: &Quantity,
    precision: f64,
) -> Result<Determination, OracleXError> {
    if !precision.is_finite() || precision < 0.0 {
        return Err(OracleXError::NonFinite {
            field: "threshold_call::precision",
            value: precision,
        });
    }
    let compatible = comparable(quantity, threshold);
    if !compatible.is_supported() {
        return Ok(compatible);
    }
    let distance = (quantity.value - threshold.value).abs();
    if distance <= precision {
        return Ok(Determination::unresolved(
            "a measurement precise enough to separate the value from the cut",
            format!(
                "{} sits {distance} from the cut at {}, within the declared precision {precision}",
                quantity.describe(),
                threshold.describe()
            ),
        ));
    }
    Ok(Determination::supported(
        EvidenceTier::Deterministic,
        format!(
            "{} is {} the cut at {} by more than {precision}",
            quantity.describe(),
            if quantity.value > threshold.value {
                "above"
            } else {
                "below"
            },
            threshold.describe()
        ),
    ))
}
