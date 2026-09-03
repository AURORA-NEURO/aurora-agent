//! Epistemic P32 local single-study research-copilot evidence-closure feature F03.
use super::evidence_closure_support::{qualify,manifest,EvidenceClosureCard7,EvidenceClosureRequest4};
const FEATURE_ID:&str="AFA-epistemic-P32-F03";const CONTRACT_VERSION:&str="epistemic-local-evidence-closure-research-copilot/1.0";
pub fn epistemic_local_evidence_closure_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","research-copilot")}
pub fn qualify_epistemic_local_evidence_closure_research_copilot(request:&EvidenceClosureRequest4)->Result<EvidenceClosureCard7,super::evidence_closure_support::EvidenceClosureError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","research-copilot")}
