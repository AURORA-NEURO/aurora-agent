//! Round-trip checks: is the view still traceable to what it came from?
//!
//! 43.01's third non-negotiable invariant is that "every projection is reversible to stable source
//! handles even when it is not information-lossless". Lossy is fine; unreachable is not. A reader
//! who sees a node must be able to name the object in the compiled region it stands for and go
//! read it.
//!
//! This module turns that into two checks a test or a CI gate can run:
//!
//! - [`obstructions_survive`] — every unresolved obligation and every oracle witness is reachable
//!   from the view. This one holds for all four projections and is additionally enforced at render
//!   time by [`crate::FidelityLedger`]; the check here is the independent confirmation, computed
//!   from the rendered body rather than from what the projection *claimed* to carry.
//! - [`evidence_survives`] — every delivered evidence capsule is reachable. This one legitimately
//!   fails for the timeline, whose subject is events rather than evidence, and the failure is
//!   visible in that view's loss ledger rather than hidden.
//!
//! Note what is *not* checked: that the view can be inverted back into a Decision Section. It
//! cannot, and 43.01 does not ask for that — only that the handles survive.

use crate::error::ProjectionError;
use crate::graph::{GraphBody, GraphProjection};
use crate::hypergraph::{HypergraphBody, HypergraphProjection};
use crate::identity::{conflict_id, obligation_id};
use crate::provenance::{BoundSection, ProjectionSource};
use crate::table::{TableBody, TableProjection};
use crate::timeline::{TimelineBody, TimelineProjection};
use crate::view::{ProjectedBody, ProjectionKind, View};
use bioprism_section::DecisionSection;
use bioprism_world::CausalEvent;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Which expected handles a view exposes and which it lost.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandleCoverage {
    pub recovered: Vec<String>,
    pub missing: Vec<String>,
}

impl HandleCoverage {
    pub fn is_complete(&self) -> bool {
        self.missing.is_empty()
    }

    fn of(expected: &BTreeSet<String>, exposed: &BTreeSet<String>) -> Self {
        let (recovered, missing): (Vec<String>, Vec<String>) = expected
            .iter()
            .cloned()
            .partition(|handle| exposed.contains(handle));
        HandleCoverage { recovered, missing }
    }
}

/// Handles of every obstruction in the section: obligations and oracle witnesses.
pub fn obstruction_handles(section: &DecisionSection) -> BTreeSet<String> {
    let mut handles: BTreeSet<String> = section
        .unresolved_obligations
        .iter()
        .enumerate()
        .map(|(index, obligation)| obligation_id(index, obligation))
        .collect();
    handles.extend(
        section
            .oracle
            .witnesses
            .iter()
            .enumerate()
            .map(|(index, witness)| conflict_id(index, witness)),
    );
    handles
}

/// Handles of every evidence capsule the section delivered.
pub fn evidence_handles(section: &DecisionSection) -> BTreeSet<String> {
    section
        .selected_evidence
        .iter()
        .map(|capsule| capsule.id.clone())
        .collect()
}

/// Confirms every obstruction is reachable from the rendered body.
pub fn obstructions_survive<B: ProjectedBody>(
    section: &DecisionSection,
    view: &View<B>,
) -> HandleCoverage {
    HandleCoverage::of(&obstruction_handles(section), &view.stable_handles())
}

/// Confirms every delivered evidence capsule is reachable from the rendered body.
pub fn evidence_survives<B: ProjectedBody>(
    section: &DecisionSection,
    view: &View<B>,
) -> HandleCoverage {
    HandleCoverage::of(&evidence_handles(section), &view.stable_handles())
}

/// All four projections of one compiled region, under one bound provenance.
///
/// 43.01 lists graph, hypergraph, timeline and table together because they are alternative
/// readings of the same object, not a pipeline. Producing them from one [`ProjectionSource`] is
/// what makes them comparable: any disagreement between them is a bug in a projection, never a
/// difference in what was compiled.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProjectionBundle {
    pub graph: View<GraphBody>,
    pub hypergraph: View<HypergraphBody>,
    pub timeline: View<TimelineBody>,
    pub table: View<TableBody>,
}

/// Projects a region four ways from a detached source.
///
/// `events` comes from the world rather than the section, because 43.25 does not put the causal
/// event structure in a Decision Section. Pass an empty slice when no event structure is at hand;
/// the timeline is then empty and says so, rather than inventing one from evidence timestamps.
///
/// The drift check runs once, not once per projection. Promoting the detached source to a
/// [`BoundSection`] is what earns that: the borrow it takes lasts across all four renders, so the
/// three further recomputations would have been re-answering a question the borrow already
/// settles. The check itself is unchanged — a section mutated since binding is still refused with
/// [`ProjectionError::SectionMutatedAfterBinding`], now before the first view is built rather than
/// during it.
pub fn project_all(
    section: &DecisionSection,
    events: &[CausalEvent],
    source: ProjectionSource,
) -> Result<ProjectionBundle, ProjectionError> {
    BoundSection::rebind(section, source)?.project_all(events)
}

impl BoundSection<'_> {
    /// Projects this region four ways under one live binding.
    ///
    /// No section digest at all: [`BoundSection`] already holds the section, so the four views are
    /// readings of the same bytes by construction rather than by four repeated hashes.
    pub fn project_all(&self, events: &[CausalEvent]) -> Result<ProjectionBundle, ProjectionError> {
        Ok(ProjectionBundle {
            graph: self.project(&GraphProjection::new())?,
            hypergraph: self.project(&HypergraphProjection::new())?,
            timeline: self.project(&TimelineProjection::new(events))?,
            table: self.project(&TableProjection::new())?,
        })
    }
}

impl ProjectionBundle {
    /// Whether every projection kept every obstruction reachable.
    pub fn obstructions_survive_everywhere(&self, section: &DecisionSection) -> bool {
        obstructions_survive(section, &self.graph).is_complete()
            && obstructions_survive(section, &self.hypergraph).is_complete()
            && obstructions_survive(section, &self.timeline).is_complete()
            && obstructions_survive(section, &self.table).is_complete()
    }

    /// The four loss ledgers, so a caller can show a reader what each view gave up.
    pub fn fidelity_summary(&self) -> Vec<(ProjectionKind, usize)> {
        vec![
            (self.graph.kind(), self.graph.fidelity().total_dropped()),
            (
                self.hypergraph.kind(),
                self.hypergraph.fidelity().total_dropped(),
            ),
            (
                self.timeline.kind(),
                self.timeline.fidelity().total_dropped(),
            ),
            (self.table.kind(), self.table.fidelity().total_dropped()),
        ]
    }
}

/// How many times one section is canonicalised and hashed, per projection path.
///
/// These are the tests that keep the optimisation honest. The claim is not "the code looks like it
/// hashes once" but "this path called the digest function exactly N times", counted at the one
/// function every section digest in this crate goes through.
#[cfg(test)]
mod digest_cost {
    use super::*;
    use crate::provenance::section_digest_calls;
    use crate::view::ProjectRegion;
    use bioprism_section::{
        Backend, CertificateProfile, ContextCertificate, EvidenceCapsule, LeakageWitness,
        OmissionManifest, OracleVerdict, PlanDescriptor, ReferenceOmissions, SourceHashes,
        UnresolvedObligation,
    };
    use serde_json::json;

    /// A blocked, contradicted region: enough structure that all four projections do real work and
    /// the loss ledger has an obstruction it must carry.
    fn section() -> DecisionSection {
        DecisionSection {
            world_id: "world.digest_cost".into(),
            query_id: "query.digest_cost".into(),
            decision_time: "2025-02-15T00:00:00Z".into(),
            goal: "count how often one section is digested".into(),
            selected_evidence: vec![EvidenceCapsule::from_raw_fact(&json!({
                "id": "fact.split_assignment",
                "provides": "split_assignment",
                "value": { "train": ["S001"], "test": ["S002"] },
                "scope": { "cohort": "C-01" },
                "tags": ["identity"],
                "provenance": ["doc://split.csv#L1"],
            }))],
            selected_factors: vec![json!({
                "id": "factor.identity_check",
                "kind": "deterministic_rule",
                "inputs": ["split_assignment", "cohort_id"],
                "outputs": ["identity_ok"],
            })],
            oracle: OracleVerdict::new(
                "split_integrity",
                vec![LeakageWitness::PreprocessingLeakage {
                    detail: "normalisation was fit across subjects before the split".into(),
                }],
            ),
            unresolved_obligations: vec![UnresolvedObligation::Obstructed {
                detail: "evidence disagreed and no gluing was possible".into(),
            }],
            refinement_frontier: vec![],
        }
    }

    /// Hashes the section directly rather than through `section_digest`, so building the fixture
    /// never lands in the count the tests are about to read.
    fn certificate_for(section: &DecisionSection) -> ContextCertificate {
        ContextCertificate {
            world_id: section.world_id.clone(),
            query_id: section.query_id.clone(),
            selected_facts: vec!["fact.split_assignment".into()],
            selected_factors: vec!["factor.identity_check".into()],
            protected_closure: vec!["split_assignment".into()],
            omissions: ReferenceOmissions {
                total_facts: 1,
                exploratory_facts: 0,
                classification: "no_omitted_fact_can_change_the_decision".into(),
                inaccessible_selected_before_cut: vec![],
            },
            plan: PlanDescriptor {
                backend: Backend::BackwardFactorSliceReference,
                compiled_factor_count: 1,
                compiled_fact_count: 1,
                total_factor_count: 1,
                total_fact_count: 1,
                max_selected_factor_arity: 3,
                fallback: None,
            },
            oracle: section.oracle.clone(),
            source_hashes: SourceHashes {
                world_sha256: "0".repeat(64),
                query_sha256: "1".repeat(64),
                decision_section_sha256: section
                    .content_hash()
                    .expect("section digests")
                    .as_str()
                    .to_string(),
            },
            limitations: vec!["digest-cost fixture".into()],
            manifest: OmissionManifest::default(),
        }
    }

    #[test]
    fn a_live_binding_digests_the_section_once_for_the_whole_bundle() {
        let section = section();
        let certificate = certificate_for(&section);

        section_digest_calls::reset();
        let bound = BoundSection::bind(&section, &certificate, CertificateProfile::Reference)
            .expect("the certificate attests this section");
        let bundle = bound.project_all(&[]).expect("projects four ways");

        assert_eq!(
            section_digest_calls::count(),
            1,
            "the only digest a live binding needs is the one that checks the certificate"
        );
        assert!(bundle.obstructions_survive_everywhere(&section));
    }

    #[test]
    fn a_detached_source_costs_one_drift_check_for_the_bundle_not_one_per_projection() {
        let section = section();
        let certificate = certificate_for(&section);

        section_digest_calls::reset();
        let source = ProjectionSource::bind(&section, &certificate, CertificateProfile::Reference)
            .expect("the certificate attests this section");
        project_all(&section, &[], source).expect("projects four ways");

        assert_eq!(
            section_digest_calls::count(),
            2,
            "one digest to bind, one to re-establish the binding, none per projection"
        );
    }

    #[test]
    fn four_separate_projections_from_a_detached_source_still_pay_a_guard_each() {
        let section = section();
        let certificate = certificate_for(&section);

        section_digest_calls::reset();
        let source = ProjectionSource::bind(&section, &certificate, CertificateProfile::Reference)
            .expect("the certificate attests this section");
        GraphProjection::new()
            .project(&section, source.clone())
            .expect("graph");
        HypergraphProjection::new()
            .project(&section, source.clone())
            .expect("hypergraph");
        TimelineProjection::new(&[])
            .project(&section, source.clone())
            .expect("timeline");
        TableProjection::new()
            .project(&section, source)
            .expect("table");

        assert_eq!(
            section_digest_calls::count(),
            5,
            "a caller who lets the binding lapse between projections buys the check back each time"
        );
    }
}
