//! RFC 3339 timestamps.
//!
//! Deliberately dependency-free and independent of the host timezone. Timestamps require an
//! explicit UTC or numeric offset and are normalized to absolute nanosecond instants. The parser
//! rejects unknown (`-00:00`) offsets, malformed offsets, and precision beyond nanoseconds rather
//! than guessing or truncating. Comparison is on the absolute instant, so an offset-bearing
//! timestamp and its UTC equivalent compare equal.

use crate::error::TimeError;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Timestamp {
    nanos_utc: i128,
}

impl Timestamp {
    pub const fn from_nanos_utc(nanos_utc: i128) -> Self {
        Timestamp { nanos_utc }
    }

    pub const fn as_nanos_utc(self) -> i128 {
        self.nanos_utc
    }

    pub fn parse(input: &str) -> Result<Self, TimeError> {
        let bad = || TimeError::Malformed(input.to_string());
        let bytes = input.as_bytes();
        if bytes.len() < 19 {
            return Err(bad());
        }

        let year: i64 = input
            .get(0..4)
            .ok_or_else(bad)?
            .parse()
            .map_err(|_| bad())?;
        if [0..4, 5..7, 8..10, 11..13, 14..16, 17..19]
            .into_iter()
            .any(|range| {
                bytes
                    .get(range)
                    .is_none_or(|part| !part.iter().all(u8::is_ascii_digit))
            })
        {
            return Err(bad());
        }
        expect(bytes, 4, b'-', input)?;
        let month: u32 = input
            .get(5..7)
            .ok_or_else(bad)?
            .parse()
            .map_err(|_| bad())?;
        expect(bytes, 7, b'-', input)?;
        let day: u32 = input
            .get(8..10)
            .ok_or_else(bad)?
            .parse()
            .map_err(|_| bad())?;
        if bytes[10] != b'T' && bytes[10] != b't' && bytes[10] != b' ' {
            return Err(bad());
        }
        let hour: i64 = input
            .get(11..13)
            .ok_or_else(bad)?
            .parse()
            .map_err(|_| bad())?;
        expect(bytes, 13, b':', input)?;
        let minute: i64 = input
            .get(14..16)
            .ok_or_else(bad)?
            .parse()
            .map_err(|_| bad())?;
        expect(bytes, 16, b':', input)?;
        let second: i64 = input
            .get(17..19)
            .ok_or_else(bad)?
            .parse()
            .map_err(|_| bad())?;

        if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
            return Err(TimeError::OutOfRange(input.to_string()));
        }
        if !(0..=23).contains(&hour) || !(0..=59).contains(&minute) || !(0..=59).contains(&second) {
            return Err(TimeError::OutOfRange(input.to_string()));
        }

        let mut cursor = 19;
        let mut subsec_nanos: i128 = 0;
        if bytes.get(cursor) == Some(&b'.') || bytes.get(cursor) == Some(&b',') {
            cursor += 1;
            let start = cursor;
            while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
                cursor += 1;
            }
            if cursor == start {
                return Err(bad());
            }
            let digits = &input[start..cursor];
            if digits.len() > 9 {
                return Err(bad());
            }
            let scale = 10i128.pow((9 - digits.len()) as u32);
            subsec_nanos = digits.parse::<i128>().map_err(|_| bad())? * scale;
        }

        let offset_seconds: i64 = match bytes.get(cursor) {
            None => return Err(bad()),
            Some(b'Z') | Some(b'z') => {
                cursor += 1;
                0
            }
            Some(sign @ (b'+' | b'-')) => {
                let negative = *sign == b'-';
                let has_colon = bytes.get(cursor + 3) == Some(&b':');
                let expected_len = if has_colon { 6 } else { 5 };
                if bytes.len() - cursor != expected_len {
                    return Err(bad());
                }
                let off_hour: i64 = input
                    .get(cursor + 1..cursor + 3)
                    .ok_or_else(bad)?
                    .parse()
                    .map_err(|_| bad())?;
                let minute_start = if has_colon { cursor + 4 } else { cursor + 3 };
                let off_minute: i64 = input
                    .get(minute_start..minute_start + 2)
                    .ok_or_else(bad)?
                    .parse()
                    .map_err(|_| bad())?;
                let offset_digits = [cursor + 1..cursor + 3, minute_start..minute_start + 2];
                if offset_digits.into_iter().any(|range| {
                    bytes
                        .get(range)
                        .is_none_or(|part| !part.iter().all(u8::is_ascii_digit))
                }) || off_hour > 23
                    || !(0..=59).contains(&off_minute)
                {
                    return Err(TimeError::OutOfRange(input.to_string()));
                }
                if negative && off_hour == 0 && off_minute == 0 {
                    return Err(bad());
                }
                cursor = minute_start + 2;
                let magnitude = off_hour * 3600 + off_minute * 60;
                if negative {
                    -magnitude
                } else {
                    magnitude
                }
            }
            Some(_) => return Err(bad()),
        };

        if cursor != bytes.len() {
            return Err(bad());
        }

        let days = days_from_civil(year, month, day);
        let seconds = days * 86_400 + hour * 3600 + minute * 60 + second - offset_seconds;
        Ok(Timestamp {
            nanos_utc: seconds as i128 * 1_000_000_000 + subsec_nanos,
        })
    }

    pub fn to_rfc3339(self) -> String {
        let total_seconds = self.nanos_utc.div_euclid(1_000_000_000) as i64;
        let subsec = self.nanos_utc.rem_euclid(1_000_000_000) as u32;
        let days = total_seconds.div_euclid(86_400);
        let secs_of_day = total_seconds.rem_euclid(86_400);
        let (year, month, day) = civil_from_days(days);
        let base = format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
            year,
            month,
            day,
            secs_of_day / 3600,
            (secs_of_day % 3600) / 60,
            secs_of_day % 60
        );
        if subsec == 0 {
            format!("{base}Z")
        } else {
            format!("{base}.{:09}Z", subsec)
        }
    }
}

fn expect(bytes: &[u8], index: usize, expected: u8, input: &str) -> Result<(), TimeError> {
    if bytes.get(index) == Some(&expected) {
        Ok(())
    } else {
        Err(TimeError::Malformed(input.to_string()))
    }
}

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m = month as i64;
    let d = day as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_rfc3339())
    }
}

impl From<Timestamp> for String {
    fn from(value: Timestamp) -> Self {
        value.to_rfc3339()
    }
}

impl TryFrom<String> for Timestamp {
    type Error = TimeError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Timestamp::parse(&value)
    }
}

/// A half-open instant range `[start, end)`. Either endpoint may be unbounded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Interval {
    pub start: Option<Timestamp>,
    pub end: Option<Timestamp>,
}

impl Interval {
    pub const UNBOUNDED: Interval = Interval {
        start: None,
        end: None,
    };

    pub fn contains(&self, at: Timestamp) -> bool {
        self.start.is_none_or(|s| at >= s) && self.end.is_none_or(|e| at < e)
    }

    pub fn is_empty(&self) -> bool {
        match (self.start, self.end) {
            (Some(s), Some(e)) => s >= e,
            _ => false,
        }
    }

    pub fn intersect(&self, other: &Interval) -> Interval {
        let start = match (self.start, other.start) {
            (Some(a), Some(b)) => Some(if a >= b { a } else { b }),
            (Some(a), None) => Some(a),
            (None, b) => b,
        };
        let end = match (self.end, other.end) {
            (Some(a), Some(b)) => Some(if a <= b { a } else { b }),
            (Some(a), None) => Some(a),
            (None, b) => b,
        };
        Interval { start, end }
    }

    pub fn refines(&self, coarser: &Interval) -> bool {
        let start_ok = match (self.start, coarser.start) {
            (_, None) => true,
            (Some(a), Some(b)) => a >= b,
            (None, Some(_)) => false,
        };
        let end_ok = match (self.end, coarser.end) {
            (_, None) => true,
            (Some(a), Some(b)) => a <= b,
            (None, Some(_)) => false,
        };
        start_ok && end_ok
    }
}

impl PartialOrd for Interval {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self == other {
            Some(Ordering::Equal)
        } else if self.refines(other) {
            Some(Ordering::Less)
        } else if other.refines(self) {
            Some(Ordering::Greater)
        } else {
            None
        }
    }
}
