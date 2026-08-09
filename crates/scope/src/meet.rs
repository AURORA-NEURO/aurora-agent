//! The scope meet.
//!
//! Blueprint 43.03 requires the meet to be explicit and 43.06 requires failure to glue to be a
//! *structured obstruction*, never a hidden contradiction score. Two failure modes are
//! therefore kept distinct:
//!
//! * [`Meet::Empty`] — the meet is well defined and computes to nothing (unequal exact values,
//!   disjoint sets, an empty interval). The two scopes simply do not overlap.
//! * [`Meet::Conflict`] — no meet operation is even defined, because the same dimension is
//!   bound to values of different kinds. This is a modelling error in the world, not a fact
//!   about the data, and it must not be reported as "no overlap".

use crate::key::{ScopeKey, ScopeValue};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Meet {
    Scope(ScopeKey),
    Empty {
        dimension: String,
        reason: EmptyReason,
    },
    Conflict {
        dimension: String,
        left: ScopeValue,
        right: ScopeValue,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmptyReason {
    UnequalExactValues,
    DisjointSets,
    EmptyInterval,
}

impl Meet {
    pub fn scope(&self) -> Option<&ScopeKey> {
        match self {
            Meet::Scope(key) => Some(key),
            _ => None,
        }
    }

    pub fn is_inhabited(&self) -> bool {
        matches!(self, Meet::Scope(_))
    }
}

pub fn meet(left: &ScopeKey, right: &ScopeKey) -> Meet {
    let mut result = ScopeKey::new();

    for (dimension, left_value) in left.iter() {
        match right.get(dimension) {
            None => result = result.bind(dimension, left_value.clone()),
            Some(right_value) => match meet_values(left_value, right_value) {
                ValueMeet::Value(value) => result = result.bind(dimension, value),
                ValueMeet::Empty(reason) => {
                    return Meet::Empty {
                        dimension: dimension.to_string(),
                        reason,
                    }
                }
                ValueMeet::Conflict => {
                    return Meet::Conflict {
                        dimension: dimension.to_string(),
                        left: left_value.clone(),
                        right: right_value.clone(),
                    }
                }
            },
        }
    }

    for (dimension, right_value) in right.iter() {
        if left.get(dimension).is_none() {
            result = result.bind(dimension, right_value.clone());
        }
    }

    Meet::Scope(result)
}

enum ValueMeet {
    Value(ScopeValue),
    Empty(EmptyReason),
    Conflict,
}

fn meet_values(left: &ScopeValue, right: &ScopeValue) -> ValueMeet {
    use ScopeValue::*;
    match (left, right) {
        (Exact(a), Exact(b)) => {
            if a == b {
                ValueMeet::Value(Exact(a.clone()))
            } else {
                ValueMeet::Empty(EmptyReason::UnequalExactValues)
            }
        }
        (Exact(a), OneOf(set)) | (OneOf(set), Exact(a)) => {
            if set.contains(a) {
                ValueMeet::Value(Exact(a.clone()))
            } else {
                ValueMeet::Empty(EmptyReason::DisjointSets)
            }
        }
        (OneOf(a), OneOf(b)) => {
            let intersection: BTreeSet<String> = a.intersection(b).cloned().collect();
            match intersection.len() {
                0 => ValueMeet::Empty(EmptyReason::DisjointSets),
                1 => ValueMeet::Value(Exact(
                    intersection.into_iter().next().expect("length checked"),
                )),
                _ => ValueMeet::Value(OneOf(intersection)),
            }
        }
        (Window(a), Window(b)) => {
            let intersection = a.intersect(b);
            if intersection.is_empty() {
                ValueMeet::Empty(EmptyReason::EmptyInterval)
            } else {
                ValueMeet::Value(Window(intersection))
            }
        }
        _ => ValueMeet::Conflict,
    }
}
