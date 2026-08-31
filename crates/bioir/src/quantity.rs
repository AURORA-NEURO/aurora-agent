//! Amounts of material, carrying their unit.
//!
//! Blueprint 39.05 lists "units, coordinate system, genome/reference build, orientation, and
//! scale" among the classes a context compiler may re-encode but never omit. A bare `f64` for
//! a volume is exactly that omission, so amounts travel with their unit string everywhere in
//! this crate.
//!
//! No unit ontology is implemented. 25.04 lists "unit conversion" under validation and never
//! says which system, which means any conversion table here would be this crate's invention
//! wearing the blueprint's authority. Arithmetic between differing unit strings therefore
//! fails with [`crate::LineageError::UnitMismatch`] instead of guessing.

use crate::error::LineageError;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Quantity {
    pub amount: f64,
    pub unit: String,
}

impl Quantity {
    pub fn new(amount: f64, unit: impl Into<String>) -> Self {
        Quantity {
            amount,
            unit: unit.into(),
        }
    }

    pub fn zero_like(&self) -> Self {
        Quantity {
            amount: 0.0,
            unit: self.unit.clone(),
        }
    }

    /// Adds `other` to `self`, refusing to add across units.
    ///
    /// `subject` names the material being summed so that a mass-balance failure points at a
    /// specimen rather than at an anonymous arithmetic error.
    pub fn add(&self, other: &Quantity, subject: &str) -> Result<Quantity, LineageError> {
        self.check_unit(other, subject)?;
        Ok(Quantity {
            amount: self.amount + other.amount,
            unit: self.unit.clone(),
        })
    }

    pub fn subtract(&self, other: &Quantity, subject: &str) -> Result<Quantity, LineageError> {
        self.check_unit(other, subject)?;
        Ok(Quantity {
            amount: self.amount - other.amount,
            unit: self.unit.clone(),
        })
    }

    pub fn exceeds(&self, other: &Quantity, subject: &str) -> Result<bool, LineageError> {
        self.check_unit(other, subject)?;
        Ok(self.amount > other.amount)
    }

    fn check_unit(&self, other: &Quantity, subject: &str) -> Result<(), LineageError> {
        if self.unit == other.unit {
            Ok(())
        } else {
            Err(LineageError::UnitMismatch {
                subject: subject.to_string(),
                left: self.unit.clone(),
                right: other.unit.clone(),
            })
        }
    }
}

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.amount, self.unit)
    }
}
