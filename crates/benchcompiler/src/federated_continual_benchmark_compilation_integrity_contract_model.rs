//! Benchcompiler P32 federated_continual contract_model benchmark-compilation integrity feature.
use super::benchmark_compilation_integrity_support::{manifest,compile,BenchmarkCard7,BenchmarkCompileRequest4,BenchmarkCompilationIntegrityError};
pub const FEATURE_ID:&str="AFA-benchcompiler-P32-F08";pub const CONTRACT_VERSION:&str="benchcompiler-federated_continual_benchmark_compilation_integrity_contract_model/1.0";
pub fn federated_continual_benchmark_compilation_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"federated_continual","contract_model")}
pub fn compile_federated_continual_benchmark_compilation_integrity_contract_model(request:&BenchmarkCompileRequest4)->Result<BenchmarkCard7,BenchmarkCompilationIntegrityError>{compile(request,FEATURE_ID,CONTRACT_VERSION,"federated_continual","contract_model")}
