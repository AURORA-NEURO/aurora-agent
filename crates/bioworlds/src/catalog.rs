//! The shipped slices, and the report over all of them.
//!
//! Four slices: two worlds built to close a written-down gap, and two controls with the identical
//! decisive skeleton and a single structural knob moved. The controls are the reason the numbers
//! mean anything. A separating depth of `None` on one world is a claim about that world; a
//! separating depth of `None` on one world and `Some(5)` on its twin, which differs only in
//! attachment, relay depth and tag style, is a claim about the knobs.
//!
//! Both controls are expected to come out *unfavourably* — they look like the reference world,
//! whose failure to discriminate is the negative result `docs/FINDINGS.md` publishes. They ship
//! anyway. §38's whole reason to exist is that worlds are where falsification happens, and a
//! catalogue that dropped its unflattering world would be the artefact this section is meant to
//! replace.

use crate::error::BioWorldError;
use crate::query::QueryShape;
use crate::slice::{BlockedProperty, SliceReport, StructuralCheck, VerticalSlice};
use crate::{temporal, underdetermined};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

/// Property ids from `crates/examples` that the worlds here bear on.
///
/// Strings, because `bioprism-examples` is not in this crate's dependency set;
/// `tests/backlog_ids.rs` pins them against that crate's own snake_case ids.
pub const NON_PROTECTED_TEMPORAL_WITHHOLDING: &str = "non_protected_temporal_withholding";
pub const UNDERDETERMINED_ABSTENTION: &str = "underdetermined_abstention";
pub const TEMPORAL_ACCESSIBILITY_WITHHOLDS_EVIDENCE: &str =
    "temporal_accessibility_withholds_evidence";
pub const STRUCTURAL_DISCRIMINATION_NO_USABLE_DEPTH: &str =
    "structural_discrimination_no_usable_depth";
pub const REFERENCE_WORLD_DOES_NOT_DISCRIMINATE: &str = "reference_world_does_not_discriminate";
pub const COHORT_SCALE_CONTRACT: &str = "cohort_scale_contract";

/// The trial-eligibility temporal firewall, at the discriminating corner of the knob space.
pub fn trial_eligibility_temporal_firewall() -> Result<VerticalSlice, BioWorldError> {
    let spec = temporal::TemporalFirewallSpec::discriminating();
    let world = temporal::build(&spec)?;
    let query = temporal::query(&spec);

    Ok(VerticalSlice::new(
        "trial-eligibility-temporal-firewall",
        "Trial eligibility: withholding evidence that is not also protected",
        world,
        query,
        temporal::DISTRACTOR_TAG,
    )
    .about(
        &["38.08", "43.09", "43.39"],
        "an event over a non-protected variable the target depends on lets a screening-time cut withhold real evidence while the protected closure stays complete, so temporal withholding and closure violation are separable failures in this world",
    )
    .checking(vec![
            StructuralCheck::WorldLoadsUnderReferenceSchema,
            StructuralCheck::TargetIsProducedByAFactor,
            StructuralCheck::VariableIsInTheTargetsDependencyClosure {
                variable: temporal::CENTRAL_LAB_CONFIRMATION.into(),
            },
            StructuralCheck::VariableIsEventManaged {
                variable: temporal::CENTRAL_LAB_CONFIRMATION.into(),
            },
            StructuralCheck::VariableIsNotProtected {
                variable: temporal::CENTRAL_LAB_CONFIRMATION.into(),
            },
            StructuralCheck::VariableIsWithheldAtTheCut {
                variable: temporal::CENTRAL_LAB_CONFIRMATION.into(),
            },
            StructuralCheck::VariableIsInTheTargetsDependencyClosure {
                variable: temporal::PROTOCOL_AMENDMENT_TEXT.into(),
            },
            StructuralCheck::VariableIsNotProtected {
                variable: temporal::PROTOCOL_AMENDMENT_TEXT.into(),
            },
            StructuralCheck::VariableIsWithheldAtTheCut {
                variable: temporal::PROTOCOL_AMENDMENT_TEXT.into(),
            },
            StructuralCheck::VariableIsEventManaged {
                variable: temporal::LOCAL_LAB_VALUE.into(),
            },
            StructuralCheck::VariableIsReadableAtTheCut {
                variable: temporal::LOCAL_LAB_VALUE.into(),
            },
            StructuralCheck::ProtectedClosureSurvivesTheCut,
            StructuralCheck::NoSeparatingDepthExists,
            StructuralCheck::TagCamouflageIsAtLeastPercent { percent: 100 },
            StructuralCheck::CohortIsAtBlueprintScale,
        StructuralCheck::WorldHasAtLeastThisManyFacts { count: 100 },
    ])
    .makes_exercisable(&[
        NON_PROTECTED_TEMPORAL_WITHHOLDING,
        TEMPORAL_ACCESSIBILITY_WITHHOLDS_EVIDENCE,
        STRUCTURAL_DISCRIMINATION_NO_USABLE_DEPTH,
        COHORT_SCALE_CONTRACT,
    ])
    .still_blocked(vec![
        BlockedProperty {
            property_id: NON_PROTECTED_TEMPORAL_WITHHOLDING.into(),
            reason: "the world side is built and checked structurally; the property is not demonstrated until a compile against this world shows a temporal-withholding diagnostic with an intact protected closure, and bioprism-fiber is not in this crate's dependency set".into(),
        },
        BlockedProperty {
            property_id: STRUCTURAL_DISCRIMINATION_NO_USABLE_DEPTH.into(),
            reason: "no separating radius exists on this crate's incidence metric, which is the structural precondition; showing that no walk depth is both sound and compact against a compiled verdict needs bioprism-baseline and bioprism-fiber, neither of which is a dependency here".into(),
        },
    ])
    .with_findings(&[
        "local_lab_value is event-managed, unprotected and readable at the cut, so withholding here is a property of the release schedule rather than of the tag vocabulary",
        "the two withheld variables reach the target through different checks (lab window, protocol version), so a compiler cannot satisfy the property by special-casing one factor",
    ]))
}

/// The same temporal structure at the reference world's structural corner.
pub fn trial_eligibility_reference_shaped_control() -> Result<VerticalSlice, BioWorldError> {
    let spec = temporal::TemporalFirewallSpec::reference_shaped();
    let world = temporal::build(&spec)?;
    let query = temporal::query(&spec);

    Ok(VerticalSlice::new(
        "trial-eligibility-firewall-reference-shaped-control",
        "Control: the same firewall with hub attachment, no relays and distinct tags",
        world,
        query,
        temporal::DISTRACTOR_TAG,
    )
    .about(
        &["38.08", "43.38", "43.39"],
        "the temporal claim is independent of the discrimination claim: moving attachment, relay depth and tag style restores a separating neighbourhood depth while the withheld, non-protected, decisive variables are unchanged",
    )
    .checking(vec![
        StructuralCheck::WorldLoadsUnderReferenceSchema,
        StructuralCheck::VariableIsWithheldAtTheCut {
            variable: temporal::CENTRAL_LAB_CONFIRMATION.into(),
        },
        StructuralCheck::VariableIsNotProtected {
            variable: temporal::CENTRAL_LAB_CONFIRMATION.into(),
        },
        StructuralCheck::ProtectedClosureSurvivesTheCut,
        StructuralCheck::SeparatingDepthIs { radius: 5 },
        StructuralCheck::TagCamouflageIsAtLeastPercent { percent: 0 },
        StructuralCheck::CohortIsAtBlueprintScale,
    ])
    .still_blocked(vec![BlockedProperty {
        property_id: REFERENCE_WORLD_DOES_NOT_DISCRIMINATE.into(),
        reason: "this control makes nothing exercisable and is not claimed to. A separating depth existing is a fact about neighbourhood traversal on this crate's incidence metric, and that property is about the world the distribution ships, not this one; showing that a walk at that depth reproduces a compiled selection needs bioprism-baseline and bioprism-fiber, neither of which is a dependency here".into(),
    }])
    .with_findings(&[
        "UNFAVOURABLE: this world has a separating neighbourhood depth of 5, the same radius docs/FINDINGS.md measures on the shipped reference world. It is shipped because it is the control, not because it discriminates",
        "tag camouflage is 0% by construction here, so a lexical shortcut is available as well; both of the reference world's weaknesses are reproduced together rather than one at a time",
    ]))
}

/// The post-treatment world built to leave the verdict open.
pub fn post_treatment_underdetermination() -> Result<VerticalSlice, BioWorldError> {
    let spec = underdetermined::PostTreatmentSpec::underdetermined();
    let world = underdetermined::build(&spec)?;
    let query = underdetermined::query(&spec);
    let perfusion = underdetermined::evidence_variable(
        &bioprism_onco::response::DiscriminatingEvidence::PerfusionMri,
    )?;
    let histopathology = underdetermined::evidence_variable(
        &bioprism_onco::response::DiscriminatingEvidence::Histopathology,
    )?;

    Ok(VerticalSlice::new(
        "post-treatment-underdetermination",
        "Post-treatment change: complete, readable evidence that still does not settle the verdict",
        world,
        query,
        underdetermined::DISTRACTOR_TAG,
    )
    .about(
        &["38.02", "43.28", "43.41"],
        "three hypotheses survive a complete and fully readable evidence set under a declared mutual exclusion, so the world underdetermines the verdict without being incomplete and without hiding anything behind the cut",
    )
    .checking(vec![
        StructuralCheck::WorldLoadsUnderReferenceSchema,
        StructuralCheck::TargetIsProducedByAFactor,
        StructuralCheck::AtLeastThisManyLiveHypotheses { count: 3 },
        StructuralCheck::HypothesesAreDeclaredMutuallyExclusive,
        StructuralCheck::NoSupportInputIsUnresolvable,
        StructuralCheck::NoSupportInputIsWithheldAtTheCut,
        StructuralCheck::EveryHypothesisIsOnTheDecisionPath,
        StructuralCheck::DiscriminatingEvidenceIsDeclaredUnobserved {
            variable: perfusion,
        },
        StructuralCheck::DiscriminatingEvidenceIsDeclaredUnobserved {
            variable: histopathology,
        },
        StructuralCheck::VariableIsInTheTargetsDependencyClosure {
            variable: underdetermined::CONFIRMATION_MEASUREMENT.into(),
        },
        StructuralCheck::VariableIsNotProtected {
            variable: underdetermined::CONFIRMATION_MEASUREMENT.into(),
        },
        StructuralCheck::VariableIsWithheldAtTheCut {
            variable: underdetermined::CONFIRMATION_MEASUREMENT.into(),
        },
        StructuralCheck::ProtectedClosureSurvivesTheCut,
        StructuralCheck::NoSeparatingDepthExists,
        StructuralCheck::CohortIsAtBlueprintScale,
    ])
    .makes_exercisable(&[
        NON_PROTECTED_TEMPORAL_WITHHOLDING,
        STRUCTURAL_DISCRIMINATION_NO_USABLE_DEPTH,
        COHORT_SCALE_CONTRACT,
    ])
    .still_blocked(vec![BlockedProperty {
        property_id: UNDERDETERMINED_ABSTENTION.into(),
        reason: "OracleVerdict::abstain still has no constructing path in bioprism-fiber; this world supplies the input such a path would need and bioprism_bioworlds::underdetermined::AbstentionStep enumerates the six things that path would have to do. On the v0.1 oracle this world's witness list is empty, so it would compile to valid — a wrong answer, not a missing one".into(),
    }])
    .with_findings(&[
        "the second instance of the §38.08 pattern: confirmation_measurement is withheld, unprotected and on the decision path, in a world whose primary claim is about ambiguity rather than time",
        "the four discriminating studies are protected, so their declared absence is inside the closure. A closure that dropped them would let a compiler present an underdetermined case as settled without ever seeing what it lacked",
    ]))
}

/// The same skeleton with one discriminating study collected.
pub fn post_treatment_resolved_control() -> Result<VerticalSlice, BioWorldError> {
    let spec = underdetermined::PostTreatmentSpec::resolved_control();
    let world = underdetermined::build(&spec)?;
    let query = underdetermined::query(&spec);

    Ok(VerticalSlice::new(
        "post-treatment-resolved-control",
        "Control: collecting one declared-absent study collapses the live hypothesis set",
        world,
        query,
        underdetermined::DISTRACTOR_TAG,
    )
    .about(
        &["38.02", "43.39"],
        "underdetermination in the sibling world is a property of its evidence set and not of its factor graph: with perfusion collected and nothing else changed, the surviving hypothesis count falls from three to one",
    )
    .checking(vec![
        StructuralCheck::WorldLoadsUnderReferenceSchema,
        StructuralCheck::ExactlyThisManyLiveHypotheses { count: 1 },
        StructuralCheck::HypothesesAreDeclaredMutuallyExclusive,
        StructuralCheck::NoSupportInputIsUnresolvable,
        StructuralCheck::EveryHypothesisIsOnTheDecisionPath,
        StructuralCheck::CohortIsAtBlueprintScale,
    ])
    .with_findings(&[
        "a single fact's value changes from a declared absence to an observation; the factor graph, the events, the tags and the cut are byte-identical to the sibling world's",
    ]))
}

/// Every slice this crate ships.
#[derive(Debug, Clone)]
pub struct SliceCatalog {
    slices: Vec<VerticalSlice>,
}

impl SliceCatalog {
    pub fn standard() -> Result<Self, BioWorldError> {
        SliceCatalog::from_slices(vec![
            trial_eligibility_temporal_firewall()?,
            trial_eligibility_reference_shaped_control()?,
            post_treatment_underdetermination()?,
            post_treatment_resolved_control()?,
        ])
    }

    pub fn from_slices(slices: Vec<VerticalSlice>) -> Result<Self, BioWorldError> {
        let mut seen = BTreeSet::new();
        for slice in &slices {
            if !seen.insert(slice.id.clone()) {
                return Err(BioWorldError::DuplicateSlice(slice.id.clone()));
            }
        }
        Ok(SliceCatalog { slices })
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

    pub fn get(&self, id: &str) -> Result<&VerticalSlice, BioWorldError> {
        self.slices
            .iter()
            .find(|slice| slice.id == id)
            .ok_or_else(|| BioWorldError::UnknownSlice(id.to_string()))
    }

    pub fn run(&self, id: &str) -> Result<SliceReport, BioWorldError> {
        self.get(id)?.run()
    }

    /// Runs every slice and digests the result.
    pub fn run_all(&self) -> Result<CatalogReport, BioWorldError> {
        let slices = self
            .slices
            .iter()
            .map(VerticalSlice::run)
            .collect::<Result<Vec<_>, _>>()?;

        let mut makes_exercisable: BTreeSet<String> = BTreeSet::new();
        let mut still_blocked: BTreeSet<String> = BTreeSet::new();
        for report in &slices {
            makes_exercisable.extend(report.makes_exercisable.iter().cloned());
            still_blocked.extend(
                report
                    .still_blocked
                    .iter()
                    .map(|blocked| blocked.property_id.clone()),
            );
        }

        let mut report = CatalogReport {
            slices,
            makes_exercisable: makes_exercisable.into_iter().collect(),
            still_blocked: still_blocked.into_iter().collect(),
            digest: String::new(),
        };
        report.digest = report.recompute_digest()?;
        Ok(report)
    }

    /// The query shapes, for a consumer that owns a compiler.
    pub fn query_documents(&self) -> Vec<(String, Value)> {
        self.slices
            .iter()
            .map(|slice| (slice.id.clone(), slice.query().to_document()))
            .collect()
    }

    pub fn queries(&self) -> Vec<&QueryShape> {
        self.slices.iter().map(VerticalSlice::query).collect()
    }
}

/// Every slice's report, plus the two backlog columns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogReport {
    pub slices: Vec<SliceReport>,
    /// Property ids some world here makes exercisable.
    pub makes_exercisable: Vec<String>,
    /// Property ids that stay blocked even with these worlds in hand. Both columns are reported;
    /// a catalogue that printed only the first would look like completeness.
    pub still_blocked: Vec<String>,
    pub digest: String,
}

impl CatalogReport {
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
        let mut value = serde_json::to_value(self).expect("catalog report is serialisable");
        if let Some(map) = value.as_object_mut() {
            map.remove("digest");
        }
        value
    }

    pub fn recompute_digest(&self) -> Result<String, BioWorldError> {
        ContentHash::of_value(&self.body())
            .map(|hash| hash.as_str().to_string())
            .map_err(|source| BioWorldError::Digest {
                subject: "<catalog>".into(),
                message: source.to_string(),
            })
    }

    pub fn digest_is_intact(&self) -> bool {
        self.recompute_digest()
            .is_ok_and(|recomputed| recomputed == self.digest)
    }

    pub fn render(&self) -> String {
        let mut out = format!("catalog digest: {}\n\n", self.digest);
        for report in &self.slices {
            out.push_str(&report.render());
            out.push('\n');
        }
        out.push_str(&format!(
            "makes exercisable: {}\nstill blocked:     {}\n",
            self.makes_exercisable.join(", "),
            self.still_blocked.join(", ")
        ));
        out
    }
}
