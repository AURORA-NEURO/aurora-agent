//! Projecting the crates that already implement §25 into the IR §25 asks for.
//!
//! Five of the modules this crate owns are the *typed wire representation* of behaviour another
//! crate already implements: 25.15 is `bioprism-weave`'s acts, 25.16 is its context capsules,
//! 25.18 is `bioprism-oracle`, 25.19 is `bioprism-mutation`, 25.20 is `bioprism-bundle`. The IR is
//! a projection of those, not a rival definition of them.
//!
//! The interesting case is the one where the projection is *lossy in the other direction*: the
//! blueprint names a required field that the implementing crate does not carry. Widening the IR to
//! make that field optional would hide the disagreement; inventing a value for it would fabricate
//! one. So a projection returns its result alongside a list of [`ProjectionGap`]s, each naming an
//! IR field the source could not fill and why. A caller that ignores the gaps gets an IR that is
//! honest about being partial; a conformance suite that reads them gets a list of things to fix in
//! one crate or the other.
//!
//! Gaps are data, not errors. A gap is not a failure of the projection — it is the projection
//! working correctly on a source that does not have the information.

use crate::canonical::Canonical;
use crate::error::IrError;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};

/// One IR field the source object could not supply.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProjectionGap {
    /// The blueprint module that requires the field, e.g. `"25.19"`.
    pub module: String,
    /// The IR field that went unfilled.
    pub ir_field: String,
    /// The Rust type the projection read from.
    pub source_type: String,
    /// Why the source could not fill it. Prose, because the reason is not enumerable.
    pub detail: String,
}

impl ProjectionGap {
    pub fn new(
        module: impl Into<String>,
        ir_field: impl Into<String>,
        source_type: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        ProjectionGap {
            module: module.into(),
            ir_field: ir_field.into(),
            source_type: source_type.into(),
            detail: detail.into(),
        }
    }
}

/// An IR value together with everything the source could not supply.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Projection<T> {
    pub value: T,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gaps: Vec<ProjectionGap>,
}

impl<T> Projection<T> {
    /// A projection that filled every required field.
    pub fn complete(value: T) -> Self {
        Projection {
            value,
            gaps: Vec::new(),
        }
    }

    pub fn with_gaps(value: T, gaps: Vec<ProjectionGap>) -> Self {
        Projection { value, gaps }
    }

    pub fn is_complete(&self) -> bool {
        self.gaps.is_empty()
    }

    /// The IR fields left unfilled, sorted, for a report.
    pub fn unfilled_fields(&self) -> Vec<&str> {
        let mut fields: Vec<&str> = self.gaps.iter().map(|gap| gap.ir_field.as_str()).collect();
        fields.sort_unstable();
        fields
    }
}

impl<T: Serialize> Projection<T> {
    /// The digest of the projected value alone.
    ///
    /// Deliberately excludes the gaps: two runs that projected the same source object must agree on
    /// the artefact digest even if one of them was built by a version of this crate that had learnt
    /// to name one more gap.
    pub fn value_digest(&self) -> Result<ContentHash, IrError> {
        self.value.digest()
    }
}
