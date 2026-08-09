//! The slice registry and the coverage report.
//!
//! Blueprint 19 is a *catalogue* of reference examples, and a catalogue is only useful if it is
//! enumerable: something has to be able to say what examples exist, run them all, and report the
//! result as one artefact. That is [`SliceRegistry`].
//!
//! [`CoverageReport`] is the half that matters more. Running every registered slice tells a reader
//! which claims are exercised; it says nothing at all about the claims nobody wrote a slice for,
//! and those are exactly the claims a newcomer will assume are covered. So the report enumerates
//! [`crate::Property::ALL`] — every property the architecture asserts — and splits it, listing the
//! unexercised ones with the concrete obstacle that keeps them unexercisable. A registry that
//! printed only its passing slices would be a worse artefact than no registry, because it would
//! look like completeness.

use crate::catalog;
use crate::error::ExampleError;
use crate::property::{Property, PropertyClaim};
use crate::report::SliceReport;
use crate::slice::VerticalSlice;
use bioprism_ids::{CanonicalError, ContentHash};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

/// Every registered vertical slice.
#[derive(Debug, Clone)]
pub struct SliceRegistry {
    slices: Vec<VerticalSlice>,
}

impl SliceRegistry {
    /// The slices this crate ships.
    pub fn standard() -> Self {
        SliceRegistry {
            slices: catalog::all(),
        }
    }

    /// Builds a registry from an arbitrary set, rejecting duplicate ids.
    pub fn from_slices(slices: Vec<VerticalSlice>) -> Result<Self, ExampleError> {
        let registry = SliceRegistry { slices };
        registry.validate()?;
        Ok(registry)
    }

    /// Checks that ids are unique, so a report can be attributed to exactly one slice.
    pub fn validate(&self) -> Result<(), ExampleError> {
        let mut seen = BTreeSet::new();
        for slice in &self.slices {
            if !seen.insert(slice.id.as_str()) {
                return Err(ExampleError::DuplicateSlice(slice.id.clone()));
            }
        }
        Ok(())
    }

    pub fn slices(&self) -> &[VerticalSlice] {
        &self.slices
    }

    pub fn len(&self) -> usize {
        self.slices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slices.is_empty()
    }

    pub fn ids(&self) -> Vec<&str> {
        self.slices.iter().map(|slice| slice.id.as_str()).collect()
    }

    pub fn get(&self, id: &str) -> Result<&VerticalSlice, ExampleError> {
        self.slices
            .iter()
            .find(|slice| slice.id == id)
            .ok_or_else(|| ExampleError::UnknownSlice(id.to_string()))
    }

    /// Runs one slice by id.
    pub fn run(&self, id: &str) -> Result<SliceReport, ExampleError> {
        self.get(id)?.run()
    }

    /// Which slices exercise a given property, in registration order.
    pub fn exercised_by(&self, property: Property) -> Vec<String> {
        self.slices
            .iter()
            .filter(|slice| slice.exercised().contains(&property))
            .map(|slice| slice.id.clone())
            .collect()
    }

    /// The coverage split, computed from registrations alone without running anything.
    pub fn coverage(&self) -> CoverageReport {
        let mut demonstrated = Vec::new();
        let mut unexercised = Vec::new();
        for property in Property::ALL {
            let claim = PropertyClaim::new(property, self.exercised_by(property));
            if claim.is_exercised() {
                demonstrated.push(claim);
            } else {
                unexercised.push(claim);
            }
        }
        CoverageReport {
            demonstrated,
            unexercised,
        }
    }

    /// Runs every registered slice and returns one digested artefact.
    pub fn run_all(&self) -> Result<RegistryReport, ExampleError> {
        self.validate()?;
        let slices = self
            .slices
            .iter()
            .map(VerticalSlice::run)
            .collect::<Result<Vec<_>, _>>()?;

        let mut report = RegistryReport {
            slices,
            coverage: self.coverage(),
            digest: String::new(),
        };
        report.digest = report
            .recompute_digest()
            .map_err(|source| ExampleError::Digest {
                slice: "<registry>".into(),
                source,
            })?;
        Ok(report)
    }
}

impl Default for SliceRegistry {
    fn default() -> Self {
        Self::standard()
    }
}

/// Which claims are exercised, and which are only claimed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageReport {
    pub demonstrated: Vec<PropertyClaim>,
    /// Properties the architecture asserts that no registered slice runs.
    pub unexercised: Vec<PropertyClaim>,
}

impl CoverageReport {
    pub fn total_claims(&self) -> usize {
        self.demonstrated.len() + self.unexercised.len()
    }

    /// Unexercised claims with no recorded obstacle.
    ///
    /// These are the honest embarrassments: nothing structural prevents a slice, somebody simply
    /// has not written one. Kept separate from the blocked ones because the two need different
    /// work, and lumping them together lets the merely-unwritten hide behind the genuinely
    /// unimplementable.
    pub fn unexercised_without_blocker(&self) -> Vec<&PropertyClaim> {
        self.unexercised
            .iter()
            .filter(|claim| claim.blocker.is_none())
            .collect()
    }

    pub fn contains(&self, property: Property) -> bool {
        self.demonstrated
            .iter()
            .any(|claim| claim.property == property)
    }

    /// A human-readable coverage table, both halves.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "Claimed properties: {} ({} exercised, {} not)\n\n",
            self.total_claims(),
            self.demonstrated.len(),
            self.unexercised.len()
        ));

        out.push_str("EXERCISED\n");
        for claim in &self.demonstrated {
            out.push_str(&format!(
                "  {} [{}]\n    {}\n    slices: {}\n",
                claim.id,
                claim.blueprint_modules.join(", "),
                claim.claim,
                claim.exercised_by.join(", ")
            ));
        }

        out.push_str("\nCLAIMED BUT NOT EXERCISED\n");
        for claim in &self.unexercised {
            out.push_str(&format!(
                "  {} [{}]\n    {}\n    blocked by: {}\n",
                claim.id,
                claim.blueprint_modules.join(", "),
                claim.claim,
                claim
                    .blocker
                    .as_deref()
                    .unwrap_or("nothing structural — no slice has been written")
            ));
        }
        out
    }
}

/// The result of running every registered slice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegistryReport {
    pub slices: Vec<SliceReport>,
    pub coverage: CoverageReport,
    pub digest: String,
}

impl RegistryReport {
    /// Whether every registered slice still demonstrates what it claims.
    pub fn holds(&self) -> bool {
        self.slices.iter().all(SliceReport::holds)
    }

    pub fn failing(&self) -> Vec<&SliceReport> {
        self.slices
            .iter()
            .filter(|report| !report.holds())
            .collect()
    }

    pub fn body(&self) -> Value {
        let mut value = serde_json::to_value(self).expect("registry report is serialisable");
        if let Some(map) = value.as_object_mut() {
            map.remove("digest");
        }
        value
    }

    pub fn recompute_digest(&self) -> Result<String, CanonicalError> {
        ContentHash::of_value(&self.body()).map(|hash| hash.as_str().to_string())
    }

    pub fn digest_is_intact(&self) -> bool {
        self.recompute_digest()
            .is_ok_and(|recomputed| recomputed == self.digest)
    }

    /// A summary a newcomer can read without opening the JSON.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("registry digest: {}\n\n", self.digest));
        for report in &self.slices {
            out.push_str(&format!(
                "{} {}\n    {}\n    digest: {}\n",
                if report.holds() { "PASS" } else { "FAIL" },
                report.slice_id,
                report.title,
                report.digest
            ));
            for failure in &report.failures {
                out.push_str(&format!("    ! {failure}\n"));
            }
        }
        out.push('\n');
        out.push_str(&self.coverage.render());
        out
    }
}
