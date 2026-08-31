//! Causal interpretation and replication program ownership.

use crate::glioma::catalog::{glioma_program_catalog, GliomaProgramDescriptor, GliomaProgramId};

pub mod causal_adjustment;
pub mod causal_contrast;
pub mod meta_analysis;
pub mod trajectory;

pub use causal_adjustment::{
    analyze_stratified_causal_adjustment, CausalStratumSummary, StratifiedCausalActionKind,
    StratifiedCausalAdjustment, StratifiedCausalDisposition, StratifiedCausalError,
    StratifiedCausalRequest, StratifiedObservation,
};
pub use causal_contrast::{
    analyze_glioma_causal_contrast, CausalContrastAnalysis, CausalContrastDisposition,
    CausalContrastError, CausalContrastRequest, UnitContrast,
};

pub use meta_analysis::{
    analyze_replication_meta_analysis, MetaAnalysisDisposition, MetaAnalysisError,
    MetaAnalysisRequest, MetaStudyContribution, ReplicationMetaAnalysis,
};

pub use trajectory::{
    analyze_glioma_trajectories, TrajectoryAnalysis, TrajectoryArmSummary, TrajectoryDisposition,
    TrajectoryError, TrajectoryObservation, TrajectoryRequest, UnitTrajectory,
    UnitTrajectoryDisposition,
};

pub const PROGRAM_ID: GliomaProgramId = GliomaProgramId::InterpretationReplication;

pub fn descriptor() -> GliomaProgramDescriptor {
    glioma_program_catalog()
        .into_iter()
        .find(|program| program.program_id == PROGRAM_ID)
        .expect("catalog contains P10")
}
