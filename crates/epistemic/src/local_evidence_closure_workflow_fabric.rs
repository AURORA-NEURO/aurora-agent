//! Epistemic P32 local single-study workflow-fabric evidence-closure feature F04.
use super::evidence_closure_support::{qualify,manifest,EvidenceClosureCard7,EvidenceClosureRequest4};
const FEATURE_ID:&str="AFA-epistemic-P32-F04";const CONTRACT_VERSION:&str="epistemic-local-evidence-closure-workflow-fabric/1.0";
pub fn epistemic_local_evidence_closure_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow-fabric")}
pub fn qualify_epistemic_local_evidence_closure_workflow_fabric(request:&EvidenceClosureRequest4)->Result<EvidenceClosureCard7,super::evidence_closure_support::EvidenceClosureError>{qualify(request,FEATURE_ID,CONTRACT_VERSION,"local single-study","workflow-fabric")}
