//! The Translation Spine.
//!
//! Blueprint 24.06's load-bearing sentence is the disclaimer: "The spine is a graph, not a
//! guaranteed chain. Some edges are absent, one-to-many, bidirectional, context-dependent, or
//! contradicted." A molecular finding does not reach a patient-level conclusion because the
//! nodes happen to be listed in that order; it reaches it only if somebody declared the edges
//! and said what evidence stands behind them.
//!
//! So [`TranslationSpine::transport`] refuses a path with a missing edge, refuses an edge whose
//! evidence is ungraded, and refuses a traversal carrying an extrapolation the edge prohibits.
//! When it succeeds it returns a [`TranslationCertificate`] listing every edge crossed — the
//! output 24.06 asks for, and the reason a conclusion here is auditable rather than asserted.
//!
//! Not implemented: the penalty *magnitudes*. 24.06 says BioPRISM "applies penalties" for ten
//! mismatch kinds but never says how large, how they compose across a multi-edge path, or how
//! they enter a score. Inventing numbers here would be inventing the science. This crate
//! enumerates the mismatches, records which ones an edge prohibits outright, and leaves the
//! arithmetic to whichever crate owns scoring — which is the honest division, since a penalty
//! table with no empirical backing would be the exact overclaiming 24.02 warns about.

use crate::error::TransportError;
use crate::maturity::EvidenceMaturity;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The nine nodes of blueprint 24.06.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpineNode {
    MolecularAlteration,
    PathwayOrRegulatoryProgram,
    CellStateOrBehavior,
    MulticellularEcology,
    TissueMorphologyAndFunction,
    ImagingOrPathologyPhenotype,
    PatientLevelCourse,
    InterventionResponse,
    OperationalDecision,
}

impl SpineNode {
    pub const ALL: [SpineNode; 9] = [
        SpineNode::MolecularAlteration,
        SpineNode::PathwayOrRegulatoryProgram,
        SpineNode::CellStateOrBehavior,
        SpineNode::MulticellularEcology,
        SpineNode::TissueMorphologyAndFunction,
        SpineNode::ImagingOrPathologyPhenotype,
        SpineNode::PatientLevelCourse,
        SpineNode::InterventionResponse,
        SpineNode::OperationalDecision,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            SpineNode::MolecularAlteration => "molecular alteration",
            SpineNode::PathwayOrRegulatoryProgram => "pathway or regulatory program",
            SpineNode::CellStateOrBehavior => "cell state or behaviour",
            SpineNode::MulticellularEcology => "multicellular ecology",
            SpineNode::TissueMorphologyAndFunction => "tissue morphology and function",
            SpineNode::ImagingOrPathologyPhenotype => "imaging/pathology phenotype",
            SpineNode::PatientLevelCourse => "patient-level course",
            SpineNode::InterventionResponse => "intervention response",
            SpineNode::OperationalDecision => "operational or translational decision",
        }
    }
}

/// The ten mismatches blueprint 24.06 penalizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mismatch {
    Species,
    CellLineVersusPrimaryTissue,
    PaediatricVersusAdult,
    TumourSubtype,
    AssayOrProtocol,
    TreatmentNaiveVersusTreated,
    CrossSectionalVersusLongitudinal,
    AssociationPresentedAsInterventionEffect,
    SubgroupExtrapolation,
    Temporal,
}

impl Mismatch {
    pub const ALL: [Mismatch; 10] = [
        Mismatch::Species,
        Mismatch::CellLineVersusPrimaryTissue,
        Mismatch::PaediatricVersusAdult,
        Mismatch::TumourSubtype,
        Mismatch::AssayOrProtocol,
        Mismatch::TreatmentNaiveVersusTreated,
        Mismatch::CrossSectionalVersusLongitudinal,
        Mismatch::AssociationPresentedAsInterventionEffect,
        Mismatch::SubgroupExtrapolation,
        Mismatch::Temporal,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Mismatch::Species => "species mismatch",
            Mismatch::CellLineVersusPrimaryTissue => "cell-line versus primary-tissue mismatch",
            Mismatch::PaediatricVersusAdult => "paediatric versus adult mismatch",
            Mismatch::TumourSubtype => "tumour subtype mismatch",
            Mismatch::AssayOrProtocol => "assay or protocol mismatch",
            Mismatch::TreatmentNaiveVersusTreated => "treatment-naive versus treated tissue",
            Mismatch::CrossSectionalVersusLongitudinal => {
                "cross-sectional versus longitudinal evidence"
            }
            Mismatch::AssociationPresentedAsInterventionEffect => {
                "association presented as intervention effect"
            }
            Mismatch::SubgroupExtrapolation => "subgroup extrapolation",
            Mismatch::Temporal => "temporal mismatch",
        }
    }
}

/// Which way the evidence on an edge points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportDirection {
    Supports,
    Contradicts,
    /// Both directions are attested in different contexts. A legitimate state, and one a
    /// certificate must be able to show rather than average away.
    Mixed,
}

/// One declared edge of the spine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationEdge {
    pub from: SpineNode,
    pub to: SpineNode,
    /// Biological context and population in which the edge was established.
    pub context: String,
    /// Evidence grade. `None` means nobody graded it, which is different from grading it low.
    pub maturity: Option<EvidenceMaturity>,
    pub direction: SupportDirection,
    /// Extrapolations this edge does not survive. Crossing it while carrying one of these is
    /// refused rather than penalized, because the edge's own author said it does not hold.
    #[serde(default)]
    pub prohibited: BTreeSet<Mismatch>,
    /// Moderators and confounders known to sit on the edge. Recorded for the certificate; not
    /// enforced, because "known" is not the same as "adjusted for".
    #[serde(default)]
    pub known_moderators: Vec<String>,
    #[serde(default)]
    pub replicated: bool,
}

/// A declared spine. Absent edges are the normal case, not an error in the spine.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TranslationSpine {
    edges: BTreeMap<(SpineNode, SpineNode), TranslationEdge>,
}

impl TranslationSpine {
    pub fn new() -> Self {
        TranslationSpine::default()
    }

    pub fn declare(mut self, edge: TranslationEdge) -> Self {
        self.edges.insert((edge.from, edge.to), edge);
        self
    }

    pub fn edge(&self, from: SpineNode, to: SpineNode) -> Option<&TranslationEdge> {
        self.edges.get(&(from, to))
    }

    /// Walks a claim along `path`, carrying `mismatches`.
    ///
    /// Adjacency in [`SpineNode::ALL`] grants nothing: an edge must have been declared. This is
    /// the difference between a spine and a ladder, and it is why a molecular result cannot
    /// reach intervention response just by being listed to the left of it.
    pub fn transport(
        &self,
        path: &[SpineNode],
        mismatches: &BTreeSet<Mismatch>,
    ) -> Result<TranslationCertificate, TransportError> {
        let mut traversed = Vec::new();
        for window in path.windows(2) {
            let (from, to) = (window[0], window[1]);
            let edge = self
                .edge(from, to)
                .ok_or(TransportError::NoEdge {
                    from: from.as_str(),
                    to: to.as_str(),
                })?;
            let maturity = edge.maturity.ok_or(TransportError::UngradedEdge {
                from: from.as_str(),
                to: to.as_str(),
            })?;
            if let Some(blocked) = edge.prohibited.intersection(mismatches).next() {
                return Err(TransportError::ProhibitedExtrapolation {
                    mismatch: blocked.as_str(),
                    from: from.as_str(),
                    to: to.as_str(),
                });
            }
            traversed.push(TraversedEdge {
                from,
                to,
                maturity,
                direction: edge.direction,
                replicated: edge.replicated,
                unresolved_moderators: edge.known_moderators.clone(),
            });
        }
        Ok(TranslationCertificate {
            traversed,
            carried_mismatches: mismatches.clone(),
        })
    }

    /// The weakest edge on a path, by rung.
    ///
    /// 24.06 lists "identify the weakest edge in a claimed mechanism" as an evaluation family.
    /// Note that this *does* compare maturity rungs numerically, which the ladder does not
    /// generally license — it is defensible only because all edges here answer the same
    /// question about the same claim, which is the condition 24.12 attaches to comparison.
    pub fn weakest_edge(certificate: &TranslationCertificate) -> Option<&TraversedEdge> {
        certificate
            .traversed
            .iter()
            .min_by_key(|edge| edge.maturity.rung())
    }
}

/// One crossed edge, as recorded in a certificate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraversedEdge {
    pub from: SpineNode,
    pub to: SpineNode,
    pub maturity: EvidenceMaturity,
    pub direction: SupportDirection,
    pub replicated: bool,
    #[serde(default)]
    pub unresolved_moderators: Vec<String>,
}

/// The translation certificate of 24.06: every traversed edge, its grade, and what was carried
/// across. Produced only by [`TranslationSpine::transport`], so it cannot claim an edge that was
/// never checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationCertificate {
    pub traversed: Vec<TraversedEdge>,
    pub carried_mismatches: BTreeSet<Mismatch>,
}

impl TranslationCertificate {
    pub fn covers(&self, from: SpineNode, to: SpineNode) -> bool {
        self.traversed
            .iter()
            .any(|edge| edge.from == from && edge.to == to)
    }

    /// Every moderator left unresolved anywhere on the path, for the "unresolved assumption"
    /// line 24.06 requires the certificate to carry.
    pub fn unresolved_assumptions(&self) -> Vec<&str> {
        self.traversed
            .iter()
            .flat_map(|edge| edge.unresolved_moderators.iter().map(String::as_str))
            .collect()
    }
}

/// A cross-scale conclusion together with the certificate that licenses it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportedClaim {
    pub claim: String,
    pub certificate: TranslationCertificate,
}

impl TransportedClaim {
    /// Binds a conclusion to a certificate, refusing when the certificate does not in fact
    /// cover the path the claim travelled. A certificate for a different route is worse than
    /// none, because it looks like diligence.
    pub fn admit(
        claim: impl Into<String>,
        path: &[SpineNode],
        certificate: TranslationCertificate,
    ) -> Result<Self, TransportError> {
        let required = path.len().saturating_sub(1);
        if required == 0 {
            return Err(TransportError::MissingCertificate { count: 0 });
        }
        if certificate.traversed.len() != required {
            return Err(TransportError::MissingCertificate { count: required });
        }
        for window in path.windows(2) {
            if !certificate.covers(window[0], window[1]) {
                return Err(TransportError::CertificateOmitsEdge {
                    from: window[0].as_str(),
                    to: window[1].as_str(),
                });
            }
        }
        Ok(TransportedClaim {
            claim: claim.into(),
            certificate,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge(from: SpineNode, to: SpineNode, maturity: EvidenceMaturity) -> TranslationEdge {
        TranslationEdge {
            from,
            to,
            context: "adult diffuse glioma".to_string(),
            maturity: Some(maturity),
            direction: SupportDirection::Supports,
            prohibited: BTreeSet::new(),
            known_moderators: vec![],
            replicated: true,
        }
    }

    fn spine() -> TranslationSpine {
        TranslationSpine::new()
            .declare(edge(
                SpineNode::MolecularAlteration,
                SpineNode::PathwayOrRegulatoryProgram,
                EvidenceMaturity::ExperimentalPerturbation,
            ))
            .declare(edge(
                SpineNode::PathwayOrRegulatoryProgram,
                SpineNode::CellStateOrBehavior,
                EvidenceMaturity::ExploratorySingleDataset,
            ))
    }

    #[test]
    fn adjacency_in_the_node_list_does_not_by_itself_license_a_traversal() {
        let err = TranslationSpine::new()
            .transport(
                &[
                    SpineNode::MolecularAlteration,
                    SpineNode::PathwayOrRegulatoryProgram,
                ],
                &BTreeSet::new(),
            )
            .unwrap_err();
        assert_eq!(
            err,
            TransportError::NoEdge {
                from: "molecular alteration",
                to: "pathway or regulatory program"
            }
        );
    }

    #[test]
    fn a_molecular_finding_cannot_reach_patient_course_without_the_intervening_edges() {
        let err = spine()
            .transport(
                &[
                    SpineNode::MolecularAlteration,
                    SpineNode::PatientLevelCourse,
                ],
                &BTreeSet::new(),
            )
            .unwrap_err();
        assert!(matches!(err, TransportError::NoEdge { .. }));
    }

    #[test]
    fn an_ungraded_edge_is_refused_rather_than_treated_as_weak_evidence() {
        let mut ungraded = edge(
            SpineNode::CellStateOrBehavior,
            SpineNode::MulticellularEcology,
            EvidenceMaturity::ExploratorySingleDataset,
        );
        ungraded.maturity = None;
        let spine = spine().declare(ungraded);
        let err = spine
            .transport(
                &[
                    SpineNode::CellStateOrBehavior,
                    SpineNode::MulticellularEcology,
                ],
                &BTreeSet::new(),
            )
            .unwrap_err();
        assert!(matches!(err, TransportError::UngradedEdge { .. }));
    }

    #[test]
    fn an_edge_prohibiting_species_extrapolation_refuses_a_traversal_carrying_one() {
        let mut restricted = edge(
            SpineNode::CellStateOrBehavior,
            SpineNode::TissueMorphologyAndFunction,
            EvidenceMaturity::ExperimentalPerturbation,
        );
        restricted.prohibited.insert(Mismatch::Species);
        let spine = spine().declare(restricted);
        let err = spine
            .transport(
                &[
                    SpineNode::CellStateOrBehavior,
                    SpineNode::TissueMorphologyAndFunction,
                ],
                &[Mismatch::Species].into_iter().collect(),
            )
            .unwrap_err();
        assert_eq!(
            err,
            TransportError::ProhibitedExtrapolation {
                mismatch: "species mismatch",
                from: "cell state or behaviour",
                to: "tissue morphology and function"
            }
        );
    }

    #[test]
    fn a_successful_traversal_produces_a_certificate_naming_every_edge_crossed() {
        let certificate = spine()
            .transport(
                &[
                    SpineNode::MolecularAlteration,
                    SpineNode::PathwayOrRegulatoryProgram,
                    SpineNode::CellStateOrBehavior,
                ],
                &BTreeSet::new(),
            )
            .unwrap();
        assert_eq!(certificate.traversed.len(), 2);
        assert!(certificate.covers(
            SpineNode::MolecularAlteration,
            SpineNode::PathwayOrRegulatoryProgram
        ));
        assert!(certificate.covers(
            SpineNode::PathwayOrRegulatoryProgram,
            SpineNode::CellStateOrBehavior
        ));
    }

    #[test]
    fn the_weakest_edge_of_a_path_is_the_one_a_reviewer_should_attack_first() {
        let certificate = spine()
            .transport(
                &[
                    SpineNode::MolecularAlteration,
                    SpineNode::PathwayOrRegulatoryProgram,
                    SpineNode::CellStateOrBehavior,
                ],
                &BTreeSet::new(),
            )
            .unwrap();
        let weakest = TranslationSpine::weakest_edge(&certificate).unwrap();
        assert_eq!(weakest.from, SpineNode::PathwayOrRegulatoryProgram);
        assert_eq!(weakest.maturity, EvidenceMaturity::ExploratorySingleDataset);
    }

    #[test]
    fn a_certificate_for_a_different_route_does_not_license_a_claim() {
        let short = spine()
            .transport(
                &[
                    SpineNode::MolecularAlteration,
                    SpineNode::PathwayOrRegulatoryProgram,
                ],
                &BTreeSet::new(),
            )
            .unwrap();
        let err = TransportedClaim::admit(
            "pathway activation drives the observed cell state",
            &[
                SpineNode::MolecularAlteration,
                SpineNode::PathwayOrRegulatoryProgram,
                SpineNode::CellStateOrBehavior,
            ],
            short,
        )
        .unwrap_err();
        assert_eq!(err, TransportError::MissingCertificate { count: 2 });
    }

    #[test]
    fn a_single_node_path_transports_nothing_and_cannot_be_certified() {
        let certificate = spine()
            .transport(&[SpineNode::MolecularAlteration], &BTreeSet::new())
            .unwrap();
        assert!(certificate.traversed.is_empty());
        assert!(
            TransportedClaim::admit("x", &[SpineNode::MolecularAlteration], certificate).is_err()
        );
    }

    #[test]
    fn unresolved_moderators_survive_into_the_certificate() {
        let mut moderated = edge(
            SpineNode::MolecularAlteration,
            SpineNode::PathwayOrRegulatoryProgram,
            EvidenceMaturity::ExternalOrFederatedValidation,
        );
        moderated.known_moderators = vec!["co-occurring 1p/19q codeletion".to_string()];
        let certificate = TranslationSpine::new()
            .declare(moderated)
            .transport(
                &[
                    SpineNode::MolecularAlteration,
                    SpineNode::PathwayOrRegulatoryProgram,
                ],
                &BTreeSet::new(),
            )
            .unwrap();
        assert_eq!(
            certificate.unresolved_assumptions(),
            vec!["co-occurring 1p/19q codeletion"]
        );
    }
}
