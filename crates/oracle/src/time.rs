//! Instants, and the validity windows built from them (31.16).
//!
//! `crates/fiber/src/oracle.rs` compares timestamps lexicographically on raw strings and records
//! that as a known limitation: for mixed UTC offsets or differing precision, lexical order is not
//! instant order, and the reference implementation it must stay bug-compatible with has the same
//! flaw. This crate is new surface, so it takes the other route — constrain the representation
//! until the cheap comparison becomes the correct one.
//!
//! [`UtcTimestamp`] admits exactly `YYYY-MM-DDTHH:MM:SSZ`. Fixed width, zero offset, no
//! fractional seconds, calendar-validated. Under that constraint byte order *is* chronological
//! order, so the derived `Ord` is sound and no date library enters the dependency graph.
//!
//! Not implemented: sub-second precision, local offsets, leap seconds, and open-ended intervals
//! keyed to anything other than wall-clock time. An oracle whose validity is keyed to a reference
//! release rather than a date should carry that release in
//! [`crate::OracleManifest::superseded_by`] instead.

use crate::error::OracleError;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A UTC instant in the fixed form `YYYY-MM-DDTHH:MM:SSZ`.
///
/// `Ord` is the derived string order, which is chronological because the form is fixed-width and
/// always Zulu. Constructing one any other way than through [`UtcTimestamp::parse`] is impossible
/// from outside this crate, so that guarantee cannot be bypassed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct UtcTimestamp(String);

impl UtcTimestamp {
    pub fn parse(value: impl Into<String>) -> Result<Self, OracleError> {
        let value = value.into();
        let bytes = value.as_bytes();

        if bytes.len() != 20 {
            return Err(OracleError::MalformedTimestamp {
                value,
                reason: "expected exactly 20 characters in the form YYYY-MM-DDTHH:MM:SSZ",
            });
        }

        for (index, byte) in bytes.iter().enumerate() {
            let well_placed = match index {
                4 | 7 => *byte == b'-',
                10 => *byte == b'T',
                13 | 16 => *byte == b':',
                19 => *byte == b'Z',
                _ => byte.is_ascii_digit(),
            };
            if !well_placed {
                return Err(OracleError::MalformedTimestamp {
                    value,
                    reason: "expected the form YYYY-MM-DDTHH:MM:SSZ with a literal Z offset",
                });
            }
        }

        let year = digits(bytes, 0, 4);
        let month = digits(bytes, 5, 7);
        let day = digits(bytes, 8, 10);
        let hour = digits(bytes, 11, 13);
        let minute = digits(bytes, 14, 16);
        let second = digits(bytes, 17, 19);

        if !(1..=12).contains(&month) {
            return Err(OracleError::MalformedTimestamp {
                value,
                reason: "month is outside 01..=12",
            });
        }
        if day < 1 || day > days_in_month(year, month) {
            return Err(OracleError::MalformedTimestamp {
                value,
                reason: "day does not exist in that month",
            });
        }
        if hour > 23 {
            return Err(OracleError::MalformedTimestamp {
                value,
                reason: "hour is outside 00..=23",
            });
        }
        if minute > 59 || second > 59 {
            return Err(OracleError::MalformedTimestamp {
                value,
                reason: "minute or second is outside 00..=59; leap seconds are not representable",
            });
        }

        Ok(UtcTimestamp(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UtcTimestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<UtcTimestamp> for String {
    fn from(value: UtcTimestamp) -> Self {
        value.0
    }
}

impl TryFrom<String> for UtcTimestamp {
    type Error = OracleError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        UtcTimestamp::parse(value)
    }
}

/// Reads an already-validated run of ASCII digits. Cannot fail, so it introduces no panic path.
fn digits(bytes: &[u8], from: usize, to: usize) -> u32 {
    bytes[from..to]
        .iter()
        .fold(0u32, |acc, b| acc * 10 + u32::from(b - b'0'))
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => 0,
    }
}

/// The interval over which an oracle's judgements are admissible (31.16).
///
/// `valid_until` is optional because some invariants genuinely do not expire — a survival table
/// whose event date precedes its index date is wrong in every future reference release. An
/// oracle that encodes a *reference standard* rather than an invariant should always set it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidityWindow {
    pub valid_from: UtcTimestamp,
    pub valid_until: Option<UtcTimestamp>,
}

impl ValidityWindow {
    pub fn new(
        valid_from: UtcTimestamp,
        valid_until: Option<UtcTimestamp>,
    ) -> Result<Self, OracleError> {
        if let Some(until) = &valid_until {
            if until < &valid_from {
                return Err(OracleError::InvertedValidityWindow {
                    valid_from: valid_from.to_string(),
                    valid_until: until.to_string(),
                });
            }
        }
        Ok(ValidityWindow {
            valid_from,
            valid_until,
        })
    }

    /// A window that opens at `valid_from` and never closes.
    pub fn open_ended(valid_from: UtcTimestamp) -> Self {
        ValidityWindow {
            valid_from,
            valid_until: None,
        }
    }

    pub fn contains(&self, at: &UtcTimestamp) -> bool {
        at >= &self.valid_from && self.valid_until.as_ref().is_none_or(|until| at <= until)
    }
}
