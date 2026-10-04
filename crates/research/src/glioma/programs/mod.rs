//! Folder ownership for the twelve executable glioma product programs.
//!
//! The files under this directory are intentionally thin ownership modules.  Their contracts and
//! algorithms live in the sibling program modules; the explicit folders keep future feature work
//! from becoming another monolithic engine file.

pub mod p01_evidence_surveillance;
pub mod p02_evidence_knowledge;
pub mod p03_multimodal_ingestion_qc;
pub mod p04_decision_context;
pub mod p05_mechanism_exploration;
pub mod p06_experiment_design;
pub mod p07_protocol_simulation;
pub mod p08_instrument_robotics;
pub mod p09_reproducible_computation;
pub mod p10_interpretation_replication;
pub mod p11_research_object_release;
pub mod p12_federated_benchmarking;
