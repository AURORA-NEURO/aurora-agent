//! Worldgen P14 F04 statistical, causal, and ML inference.
use super::interpretation_visualization_support::{self,EvidenceBackedResult4,InteractiveInterpretation1};
pub const FEATURE_ID:&str="AFA-worldgen-P14-F04"; pub const CONTRACT_VERSION:&str="worldgen-federated_continual-interpretation-visualization/1.0";
pub fn worldgen_federated_continual_interpretation_visualization_inference_manifest()->serde_json::Value{interpretation_visualization_support::manifest(FEATURE_ID,CONTRACT_VERSION,"federated continual autonomous")}
pub fn qualify_worldgen_federated_continual_interpretation_visualization_interpretation(request:&EvidenceBackedResult4)->Result<InteractiveInterpretation1,interpretation_visualization_support::InterpretationVisualizationError>{interpretation_visualization_support::qualify(request,FEATURE_ID,CONTRACT_VERSION)}
pub use interpretation_visualization_support::{InterpretationCandidate,InterpretationEvidenceState,InterpretationVisualizationError};

