//! Units, dimensions, and conversions that leave a trace.
//!
//! Blueprint 39.05 protects "units, coordinate system, genome/reference build, orientation, and
//! scale" as a non-compressible class, and 28.00 lists silent coercion among the forbidden
//! behaviours. A bare `f64` violates both: it carries no dimension to check and no record of
//! having been rescaled.
//!
//! Two checks are kept separate here on purpose.
//!
//! *Dimension* answers whether a conversion factor could exist at all. Millimetres and
//! millilitres differ in dimension, so no factor exists and the operation is refused
//! ([`crate::UnitError::DimensionMismatch`]).
//!
//! *Commensurability* answers whether a factor that does exist is meaningful. `mg/kg` and `%`
//! are both dimensionless, and a purely dimensional treatment would cheerfully rescale a
//! body-weight dose into a percentage. Ratio units therefore carry the name of the ratio they
//! express, and units of different ratio kinds never convert
//! ([`crate::UnitError::NotCommensurable`]).
//!
//! What is deliberately *not* here: no UCUM grammar, no prefix parser, no unit ontology
//! download. Blueprint 28.19 pins "reference and ontology versions" but never specifies a unit
//! system, so a general parser would be this crate's invention wearing the blueprint's
//! authority. Instead there is a closed table covering the SI-prefixed and clinical units
//! neuro-oncology actually uses, plus explicit composition. An unrecognised symbol is an error,
//! never a new unit.
//!
//! Arithmetic follows `bioprism-bioir`: [`Quantity::add`] refuses two different unit symbols
//! even when a factor exists, because an implicit rescale is a fact about the data that leaves
//! no trace. [`Quantity::convert_to`] performs the rescale and hands back a
//! [`ConversionRecord`] that says so.

use crate::error::UnitError;
use bioprism_scope::LossLedger;
use serde::{Deserialize, Serialize};
use std::fmt;

/// The base quantities this crate reasons about.
///
/// `AbsorbedDose` is a base rather than the SI-correct `L^2 T^-2` decomposition of the gray.
/// Decomposing it would make a radiotherapy dose dimensionally commensurable with a squared
/// velocity, which is true in physics and useless in a tumour board. `Entity` counts cells,
/// reads and lesions: countable things are dimensionless in SI, but keeping them separate stops
/// a cell concentration converting into a molar one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BaseDimension {
    Length,
    Mass,
    Time,
    Amount,
    AbsorbedDose,
    Entity,
}

impl BaseDimension {
    pub const ALL: [BaseDimension; 6] = [
        BaseDimension::Length,
        BaseDimension::Mass,
        BaseDimension::Time,
        BaseDimension::Amount,
        BaseDimension::AbsorbedDose,
        BaseDimension::Entity,
    ];

    /// Single-letter tag used when rendering a dimension.
    pub fn tag(self) -> &'static str {
        match self {
            BaseDimension::Length => "L",
            BaseDimension::Mass => "M",
            BaseDimension::Time => "T",
            BaseDimension::Amount => "N",
            BaseDimension::AbsorbedDose => "D",
            BaseDimension::Entity => "E",
        }
    }
}

fn is_zero(value: &i8) -> bool {
    *value == 0
}

/// A vector of integer exponents over [`BaseDimension`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Dimension {
    #[serde(default, skip_serializing_if = "is_zero")]
    pub length: i8,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub mass: i8,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub time: i8,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub amount: i8,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub absorbed_dose: i8,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub entity: i8,
}

const fn dim(
    length: i8,
    mass: i8,
    time: i8,
    amount: i8,
    absorbed_dose: i8,
    entity: i8,
) -> Dimension {
    Dimension {
        length,
        mass,
        time,
        amount,
        absorbed_dose,
        entity,
    }
}

impl Dimension {
    pub const DIMENSIONLESS: Dimension = dim(0, 0, 0, 0, 0, 0);
    pub const LENGTH: Dimension = dim(1, 0, 0, 0, 0, 0);
    pub const AREA: Dimension = dim(2, 0, 0, 0, 0, 0);
    pub const VOLUME: Dimension = dim(3, 0, 0, 0, 0, 0);
    pub const MASS: Dimension = dim(0, 1, 0, 0, 0, 0);
    pub const TIME: Dimension = dim(0, 0, 1, 0, 0, 0);
    pub const AMOUNT: Dimension = dim(0, 0, 0, 1, 0, 0);
    pub const ABSORBED_DOSE: Dimension = dim(0, 0, 0, 0, 1, 0);
    pub const ENTITY: Dimension = dim(0, 0, 0, 0, 0, 1);
    /// Mass per body-surface-area, the unit of most cytotoxic chemotherapy dosing.
    pub const MASS_PER_AREA: Dimension = dim(-2, 1, 0, 0, 0, 0);
    /// Entities per volume, as in a cell or copy concentration.
    pub const ENTITY_PER_VOLUME: Dimension = dim(-3, 0, 0, 0, 0, 1);

    pub fn exponent(self, base: BaseDimension) -> i8 {
        match base {
            BaseDimension::Length => self.length,
            BaseDimension::Mass => self.mass,
            BaseDimension::Time => self.time,
            BaseDimension::Amount => self.amount,
            BaseDimension::AbsorbedDose => self.absorbed_dose,
            BaseDimension::Entity => self.entity,
        }
    }

    pub fn is_dimensionless(self) -> bool {
        self == Dimension::DIMENSIONLESS
    }

    /// Adds exponents, refusing to wrap.
    ///
    /// Overflow is an error rather than a wrap because a wrapped exponent produces a dimension
    /// that silently compares equal to an unrelated one, which is precisely the class of bug
    /// this module exists to prevent.
    pub fn checked_mul(self, other: Dimension) -> Result<Dimension, UnitError> {
        self.combine(other, 1)
    }

    pub fn checked_div(self, other: Dimension) -> Result<Dimension, UnitError> {
        self.combine(other, -1)
    }

    pub fn checked_pow(self, exponent: i8) -> Result<Dimension, UnitError> {
        let mut out = Dimension::DIMENSIONLESS;
        for base in BaseDimension::ALL {
            let value = self
                .exponent(base)
                .checked_mul(exponent)
                .ok_or_else(|| UnitError::ExponentOverflow {
                    left: self.to_string(),
                    right: format!("^{exponent}"),
                })?;
            out.set(base, value);
        }
        Ok(out)
    }

    fn combine(self, other: Dimension, sign: i8) -> Result<Dimension, UnitError> {
        let mut out = Dimension::DIMENSIONLESS;
        for base in BaseDimension::ALL {
            let scaled = other.exponent(base).checked_mul(sign);
            let value = scaled.and_then(|s| self.exponent(base).checked_add(s));
            out.set(
                base,
                value.ok_or_else(|| UnitError::ExponentOverflow {
                    left: self.to_string(),
                    right: other.to_string(),
                })?,
            );
        }
        Ok(out)
    }

    fn set(&mut self, base: BaseDimension, value: i8) {
        match base {
            BaseDimension::Length => self.length = value,
            BaseDimension::Mass => self.mass = value,
            BaseDimension::Time => self.time = value,
            BaseDimension::Amount => self.amount = value,
            BaseDimension::AbsorbedDose => self.absorbed_dose = value,
            BaseDimension::Entity => self.entity = value,
        }
    }
}

impl fmt::Display for Dimension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_dimensionless() {
            return write!(f, "1");
        }
        let mut first = true;
        for base in BaseDimension::ALL {
            let exponent = self.exponent(base);
            if exponent == 0 {
                continue;
            }
            if !first {
                write!(f, "*")?;
            }
            first = false;
            if exponent == 1 {
                write!(f, "{}", base.tag())?;
            } else {
                write!(f, "{}^{}", base.tag(), exponent)?;
            }
        }
        Ok(())
    }
}

/// Whether a unit's factor to the base unit is exact or a stated convention.
///
/// `month` is the case that forces this distinction. Progression-free survival is reported in
/// months, months are not a fixed number of seconds, and any factor is a convention. Recording
/// which convention was assumed is the difference between a reproducible number and a number
/// that happens to agree with whatever the last tool assumed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "exactness", rename_all = "snake_case")]
pub enum Exactness {
    Exact,
    Conventional { convention: String },
}

impl Exactness {
    pub fn is_exact(&self) -> bool {
        matches!(self, Exactness::Exact)
    }

    /// The stricter of the two, so composing anything with a conventional unit stays conventional.
    fn combine(&self, other: &Exactness) -> Exactness {
        match (self, other) {
            (Exactness::Exact, Exactness::Exact) => Exactness::Exact,
            (Exactness::Conventional { convention }, Exactness::Exact)
            | (Exactness::Exact, Exactness::Conventional { convention }) => {
                Exactness::Conventional {
                    convention: convention.clone(),
                }
            }
            (
                Exactness::Conventional { convention: left },
                Exactness::Conventional { convention: right },
            ) => Exactness::Conventional {
                convention: format!("{left}; {right}"),
            },
        }
    }
}

/// What kind of quantity a unit measures, beyond its dimension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UnitKind {
    /// An ordinary dimensional unit. Commensurable with any unit of the same dimension.
    Dimensional,
    /// A ratio whose numerator and denominator cancel. `of` names what the ratio is *of*, and
    /// two ratios of different things never convert into one another.
    Ratio { of: String },
}

/// A unit: a symbol, a dimension, and a factor to the base unit of that dimension.
///
/// Base units are metre, kilogram, second, mole, gray and one entity. The factor is stored
/// rather than derived so that the closed table stays readable as a table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Unit {
    pub symbol: String,
    pub dimension: Dimension,
    pub to_base: f64,
    pub exactness: Exactness,
    pub kind: UnitKind,
}

struct UnitDef {
    symbol: &'static str,
    dimension: Dimension,
    to_base: f64,
    convention: Option<&'static str>,
    ratio_of: Option<&'static str>,
}

const fn unit_def(symbol: &'static str, dimension: Dimension, to_base: f64) -> UnitDef {
    UnitDef {
        symbol,
        dimension,
        to_base,
        convention: None,
        ratio_of: None,
    }
}

const fn conventional(
    symbol: &'static str,
    dimension: Dimension,
    to_base: f64,
    convention: &'static str,
) -> UnitDef {
    UnitDef {
        symbol,
        dimension,
        to_base,
        convention: Some(convention),
        ratio_of: None,
    }
}

const fn ratio(symbol: &'static str, to_base: f64, of: &'static str) -> UnitDef {
    UnitDef {
        symbol,
        dimension: Dimension::DIMENSIONLESS,
        to_base,
        convention: None,
        ratio_of: Some(of),
    }
}

/// The closed unit table.
///
/// Scope is neuro-oncology reporting: tumour diameters and areas in millimetres, volumes in
/// millilitres and cubic millimetres, chemotherapy dosing in mg/m², radiotherapy in gray,
/// survival in months. Anything outside the table is [`UnitError::UnknownUnit`].
const UNIT_TABLE: &[UnitDef] = &[
    unit_def("m", Dimension::LENGTH, 1.0),
    unit_def("cm", Dimension::LENGTH, 1e-2),
    unit_def("mm", Dimension::LENGTH, 1e-3),
    unit_def("um", Dimension::LENGTH, 1e-6),
    unit_def("nm", Dimension::LENGTH, 1e-9),
    unit_def("m2", Dimension::AREA, 1.0),
    unit_def("cm2", Dimension::AREA, 1e-4),
    unit_def("mm2", Dimension::AREA, 1e-6),
    unit_def("m3", Dimension::VOLUME, 1.0),
    unit_def("cm3", Dimension::VOLUME, 1e-6),
    unit_def("mm3", Dimension::VOLUME, 1e-9),
    unit_def("L", Dimension::VOLUME, 1e-3),
    unit_def("mL", Dimension::VOLUME, 1e-6),
    unit_def("uL", Dimension::VOLUME, 1e-9),
    unit_def("kg", Dimension::MASS, 1.0),
    unit_def("g", Dimension::MASS, 1e-3),
    unit_def("mg", Dimension::MASS, 1e-6),
    unit_def("ug", Dimension::MASS, 1e-9),
    unit_def("s", Dimension::TIME, 1.0),
    unit_def("min", Dimension::TIME, 60.0),
    unit_def("h", Dimension::TIME, 3600.0),
    unit_def("day", Dimension::TIME, 86_400.0),
    unit_def("week", Dimension::TIME, 604_800.0),
    conventional(
        "month",
        Dimension::TIME,
        2_629_746.0,
        "month = 30.436875 days (mean Gregorian month); calendar months are not equal",
    ),
    conventional(
        "year",
        Dimension::TIME,
        31_556_952.0,
        "year = 365.2425 days (mean Gregorian year)",
    ),
    unit_def("mol", Dimension::AMOUNT, 1.0),
    unit_def("mmol", Dimension::AMOUNT, 1e-3),
    unit_def("umol", Dimension::AMOUNT, 1e-6),
    unit_def("Gy", Dimension::ABSORBED_DOSE, 1.0),
    unit_def("cGy", Dimension::ABSORBED_DOSE, 1e-2),
    unit_def("cell", Dimension::ENTITY, 1.0),
    unit_def("mg/m2", Dimension::MASS_PER_AREA, 1e-6),
    unit_def("g/m2", Dimension::MASS_PER_AREA, 1e-3),
    unit_def("cells/uL", Dimension::ENTITY_PER_VOLUME, 1e9),
    ratio("1", 1.0, "plain fraction"),
    ratio("%", 1e-2, "plain fraction"),
    ratio("mg/kg", 1e-6, "mass per body mass"),
];

impl Unit {
    /// Looks a symbol up in the closed table.
    ///
    /// This is a lookup, not a parse: `"mm"` resolves, `"millimetre"` and `"mm "` do not.
    pub fn parse(symbol: &str) -> Result<Unit, UnitError> {
        UNIT_TABLE
            .iter()
            .find(|def| def.symbol == symbol)
            .map(Unit::from_def)
            .ok_or_else(|| UnitError::UnknownUnit {
                symbol: symbol.to_string(),
            })
    }

    fn from_def(def: &UnitDef) -> Unit {
        Unit {
            symbol: def.symbol.to_string(),
            dimension: def.dimension,
            to_base: def.to_base,
            exactness: match def.convention {
                Some(convention) => Exactness::Conventional {
                    convention: convention.to_string(),
                },
                None => Exactness::Exact,
            },
            kind: match def.ratio_of {
                Some(of) => UnitKind::Ratio { of: of.to_string() },
                None => UnitKind::Dimensional,
            },
        }
    }

    /// Every symbol the table knows, for callers that want to report the vocabulary.
    pub fn known_symbols() -> impl Iterator<Item = &'static str> {
        UNIT_TABLE.iter().map(|def| def.symbol)
    }

    pub fn is_ratio(&self) -> bool {
        matches!(self.kind, UnitKind::Ratio { .. })
    }

    /// Whether a conversion between the two units would be meaningful.
    ///
    /// Equal dimensions are necessary but not sufficient: two ratios of different things are
    /// dimensionally identical and semantically unrelated.
    pub fn commensurable_with(&self, other: &Unit) -> Result<(), UnitError> {
        if self.dimension != other.dimension {
            return Err(UnitError::DimensionMismatch {
                operation: "convert".to_string(),
                left: self.symbol.clone(),
                right: other.symbol.clone(),
                left_dimension: self.dimension,
                right_dimension: other.dimension,
            });
        }
        match (&self.kind, &other.kind) {
            (UnitKind::Dimensional, UnitKind::Dimensional) => Ok(()),
            (UnitKind::Ratio { of: left }, UnitKind::Ratio { of: right }) if left == right => {
                Ok(())
            }
            (UnitKind::Ratio { of: left }, UnitKind::Ratio { of: right }) => {
                Err(UnitError::NotCommensurable {
                    left: self.symbol.clone(),
                    right: other.symbol.clone(),
                    reason: format!("one is a ratio of {left}, the other of {right}"),
                })
            }
            _ => Err(UnitError::NotCommensurable {
                left: self.symbol.clone(),
                right: other.symbol.clone(),
                reason: "one is a ratio and the other is a dimensional quantity".to_string(),
            }),
        }
    }

    /// Composes two units by multiplication.
    ///
    /// Ratio units are rejected: `mg/kg` times `kg` is meaningful to a pharmacologist, but the
    /// table records only that the denominator is "body mass", not which body, so the result
    /// would be a mass attributed to an unidentified subject.
    pub fn checked_mul(&self, other: &Unit) -> Result<Unit, UnitError> {
        self.compose(other, "*")
    }

    pub fn checked_div(&self, other: &Unit) -> Result<Unit, UnitError> {
        self.compose(other, "/")
    }

    fn compose(&self, other: &Unit, operator: &str) -> Result<Unit, UnitError> {
        for candidate in [self, other] {
            if candidate.is_ratio() {
                return Err(UnitError::NonCompositional {
                    symbol: candidate.symbol.clone(),
                });
            }
        }
        let dimension = if operator == "*" {
            self.dimension.checked_mul(other.dimension)?
        } else {
            self.dimension.checked_div(other.dimension)?
        };
        let to_base = if operator == "*" {
            self.to_base * other.to_base
        } else {
            self.to_base / other.to_base
        };
        let symbol = format!("({}{}{})", self.symbol, operator, other.symbol);
        let composed = Unit {
            symbol,
            dimension,
            to_base,
            exactness: self.exactness.combine(&other.exactness),
            kind: UnitKind::Dimensional,
        };
        Ok(composed.prefer_table_symbol())
    }

    /// Replaces a synthesised symbol with a table symbol when one matches exactly.
    ///
    /// Keeps `mg/m2 * m2` printing as `mg` instead of `(mg/m2*m2)`. The match is on dimension
    /// and factor together, so it never renames a unit into an unrelated one.
    fn prefer_table_symbol(self) -> Unit {
        let matched = UNIT_TABLE.iter().find(|def| {
            def.ratio_of.is_none()
                && def.dimension == self.dimension
                && (def.to_base - self.to_base).abs() <= self.to_base.abs() * 1e-12
        });
        match matched {
            Some(def) => Unit {
                symbol: def.symbol.to_string(),
                ..self
            },
            None => self,
        }
    }
}

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.symbol)
    }
}

/// A number that knows what it is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Quantity {
    pub value: f64,
    pub unit: Unit,
}

/// The receipt for a conversion that actually happened.
///
/// 28.00 forbids silent coercion. A conversion is a coercion, so it produces a record; a
/// conversion whose factor rests on a convention produces a record that says which convention.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversionRecord {
    pub from: String,
    pub to: String,
    pub factor: f64,
    pub exactness: Exactness,
}

impl ConversionRecord {
    pub fn is_exact(&self) -> bool {
        self.exactness.is_exact()
    }

    /// Renders the record as a `bioprism-scope` loss ledger.
    ///
    /// An exact conversion adds nothing to the ledger, which is honest: rescaling millimetres
    /// to centimetres discards no information. A conventional conversion adds an uncertainty
    /// entry, so a downstream [`bioprism_scope::ScopeMapping`] carrying it will not be flagged
    /// as an undeclared-loss transport.
    pub fn loss_ledger(&self) -> LossLedger {
        match &self.exactness {
            Exactness::Exact => LossLedger::default(),
            Exactness::Conventional { convention } => LossLedger::default().adding_uncertainty(
                format!("{} -> {} assumes {convention}", self.from, self.to),
            ),
        }
    }
}

/// A converted quantity together with the record of its conversion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Converted {
    pub quantity: Quantity,
    pub record: ConversionRecord,
}

impl Quantity {
    pub fn new(value: f64, unit: Unit) -> Self {
        Quantity { value, unit }
    }

    /// Builds a quantity from a value and a table symbol.
    pub fn parse(value: f64, symbol: &str) -> Result<Self, UnitError> {
        Ok(Quantity {
            value,
            unit: Unit::parse(symbol)?,
        })
    }

    pub fn dimension(&self) -> Dimension {
        self.unit.dimension
    }

    /// Adds two quantities of the *same* unit.
    ///
    /// Differing dimensions and differing units fail with different errors, because they need
    /// different fixes: one is a modelling mistake, the other a missing conversion.
    pub fn add(&self, other: &Quantity) -> Result<Quantity, UnitError> {
        self.same_unit(other, "add")?;
        Ok(Quantity {
            value: self.value + other.value,
            unit: self.unit.clone(),
        })
    }

    pub fn sub(&self, other: &Quantity) -> Result<Quantity, UnitError> {
        self.same_unit(other, "subtract")?;
        Ok(Quantity {
            value: self.value - other.value,
            unit: self.unit.clone(),
        })
    }

    /// Compares two quantities of the same unit.
    pub fn compare(&self, other: &Quantity) -> Result<std::cmp::Ordering, UnitError> {
        self.same_unit(other, "compare")?;
        Ok(self
            .value
            .partial_cmp(&other.value)
            .unwrap_or(std::cmp::Ordering::Equal))
    }

    pub fn mul(&self, other: &Quantity) -> Result<Quantity, UnitError> {
        Ok(Quantity {
            value: self.value * other.value,
            unit: self.unit.checked_mul(&other.unit)?,
        })
    }

    pub fn div(&self, other: &Quantity) -> Result<Quantity, UnitError> {
        Ok(Quantity {
            value: self.value / other.value,
            unit: self.unit.checked_div(&other.unit)?,
        })
    }

    /// Rescales into `target`, returning the value *and* the receipt.
    ///
    /// The record is not optional and not a side channel: the only way to get a converted
    /// number out of this crate is to also receive the statement that a conversion occurred.
    pub fn convert_to(&self, target: &Unit) -> Result<Converted, UnitError> {
        self.unit.commensurable_with(target)?;
        let factor = self.unit.to_base / target.to_base;
        let exactness = self.unit.exactness.combine(&target.exactness);
        Ok(Converted {
            quantity: Quantity {
                value: self.value * factor,
                unit: target.clone(),
            },
            record: ConversionRecord {
                from: self.unit.symbol.clone(),
                to: target.symbol.clone(),
                factor,
                exactness,
            },
        })
    }

    fn same_unit(&self, other: &Quantity, operation: &str) -> Result<(), UnitError> {
        if self.unit.symbol == other.unit.symbol {
            return Ok(());
        }
        if self.unit.dimension != other.unit.dimension {
            return Err(UnitError::DimensionMismatch {
                operation: operation.to_string(),
                left: self.unit.symbol.clone(),
                right: other.unit.symbol.clone(),
                left_dimension: self.unit.dimension,
                right_dimension: other.unit.dimension,
            });
        }
        Err(UnitError::UnitMismatch {
            operation: operation.to_string(),
            left: self.unit.symbol.clone(),
            right: other.unit.symbol.clone(),
        })
    }
}

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.value, self.unit.symbol)
    }
}
