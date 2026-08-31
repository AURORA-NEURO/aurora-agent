//! Worldgen P16 F01 statistical, causal, and ML inference.
use super::publication_research_object_support::{self,ValidatedResearchRun2,SignedResearchObject1};
pub const FEATURE_ID:&str="AFA-worldgen-P16-F01"; pub const CONTRACT_VERSION:&str="worldgen-local-publication-research-object/1.0";
pub fn worldgen_local_publication_research_object_inference_manifest()->serde_json::Value{publication_research_object_support::manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study")}
pub fn qualify_worldgen_local_publication_research_object_release(request:&ValidatedResearchRun2)->Result<SignedResearchObject1,publication_research_object_support::PublicationResearchObjectError>{publication_research_object_support::qualify(request,FEATURE_ID,CONTRACT_VERSION)}
pub use publication_research_object_support::{ResearchObjectCandidate,ReleaseEvidenceState,PublicationResearchObjectError};

