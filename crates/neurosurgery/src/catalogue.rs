//! The closed, read-only tool catalogue and specialty routes.

use crate::{ObservationKind, Specialty, ToolCapability, ToolEffect, ToolSpec};

/// Returns the complete route for a specialty. The first three and final entries are universal.
pub fn required_capabilities(specialty: Specialty) -> Vec<ToolCapability> {
    let mut route = vec![
        ToolCapability::SafetyGate,
        ToolCapability::CaseIntegrity,
        ToolCapability::EvidenceGapScan,
    ];
    route.push(match specialty {
        Specialty::Glioma => ToolCapability::MolecularContext,
        Specialty::CranialBase => ToolCapability::CranialBaseRiskMap,
        Specialty::Craniosynostosis | Specialty::Encephalocele => {
            ToolCapability::CraniofacialDevelopment
        }
        Specialty::SpinaBifida => ToolCapability::SpinalDysraphismMap,
        Specialty::ChiariMalformation => ToolCapability::CraniocervicalJunctionMap,
    });
    route.extend([
        ToolCapability::ImagingReview,
        ToolCapability::NeuroanatomyMap,
        ToolCapability::DifferentialMatrix,
        ToolCapability::LongitudinalTrajectory,
        ToolCapability::EvidenceSynthesis,
        ToolCapability::HumanReviewHold,
    ]);
    route
}

/// The built-in tools are descriptions and deterministic checks, never effectful adapters.
pub fn tool_catalogue() -> Vec<ToolSpec> {
    ToolCapability::ALL.iter().copied().map(tool_spec).collect()
}

pub(crate) fn tool_spec(capability: ToolCapability) -> ToolSpec {
    let (purpose, required_inputs): (&str, Vec<ObservationKind>) = match capability {
        ToolCapability::SafetyGate => (
            "Check declared purpose, identifiers, and clinical-boundary posture before any domain work.",
            vec![],
        ),
        ToolCapability::CaseIntegrity => (
            "Check that observations and evidence carry explicit status and provenance identifiers.",
            vec![],
        ),
        ToolCapability::EvidenceGapScan => (
            "Compare declared specialty needs with measured, interpretable inputs without imputing missing data.",
            vec![],
        ),
        ToolCapability::ImagingReview => (
            "Inventory imaging sequences, timepoints, and limitations supplied by the caller.",
            vec![ObservationKind::Imaging],
        ),
        ToolCapability::NeuroanatomyMap => (
            "Organize caller-supplied structures, relationships, and corridor questions for review.",
            vec![ObservationKind::Neuroanatomy],
        ),
        ToolCapability::MolecularContext => (
            "Keep histology, molecular calls, assay scope, and unrun assays distinct.",
            vec![ObservationKind::Histology, ObservationKind::Molecular],
        ),
        ToolCapability::DifferentialMatrix => (
            "Create labelled research hypotheses and the checks that would discriminate them; it is not a diagnosis.",
            vec![],
        ),
        ToolCapability::LongitudinalTrajectory => (
            "Align available observations over time and expose gaps or conflicting timepoints.",
            vec![ObservationKind::LongitudinalOutcome],
        ),
        ToolCapability::CranialBaseRiskMap => (
            "Map supplied cranial-base anatomy and risk questions for specialist human review.",
            vec![ObservationKind::Imaging, ObservationKind::Neuroanatomy],
        ),
        ToolCapability::CraniofacialDevelopment => (
            "Organize craniofacial developmental and imaging observations without prescribing an intervention.",
            vec![ObservationKind::DevelopmentalTrajectory, ObservationKind::Imaging],
        ),
        ToolCapability::SpinalDysraphismMap => (
            "Organize spinal dysraphism, function, and longitudinal observations for review.",
            vec![ObservationKind::SpinalDysraphism, ObservationKind::NeurologicFunction],
        ),
        ToolCapability::CraniocervicalJunctionMap => (
            "Organize craniocervical-junction observations and imaging limitations for review.",
            vec![ObservationKind::CraniocervicalJunction, ObservationKind::Imaging],
        ),
        ToolCapability::RealDataInventory => (
            "Audit validated public glioma registry, genomic-project, study-catalog, cBioPortal molecular-profile, guideline, and PubMed abstract/index metadata without treating population records as patient findings.",
            vec![],
        ),
        ToolCapability::RealDataQuery => (
            "Query the validated public glioma bundle by stable record text, PMID, DOI, registry status, source facet, or explicit study/profile/publication relationship with bounded, source-linked hits.",
            vec![],
        ),
        ToolCapability::EvidenceSynthesis => (
            "Inventory provenance-bearing literature and separate verified, unverified, and conflicting inputs.",
            vec![],
        ),
        ToolCapability::HumanReviewHold => (
            "Freeze the result for a qualified human reviewer before any downstream use.",
            vec![],
        ),
    };
    ToolSpec {
        capability,
        label: capability.label().to_string(),
        purpose: purpose.to_string(),
        effect: ToolEffect::ReadOnly,
        required_inputs,
    }
}
