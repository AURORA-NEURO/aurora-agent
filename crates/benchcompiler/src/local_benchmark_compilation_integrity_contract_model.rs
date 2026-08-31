//! Benchcompiler P32 local contract_model benchmark-compilation integrity feature.
use super::benchmark_compilation_integrity_support::{manifest,compile,BenchmarkCard7,BenchmarkCompileRequest4,BenchmarkCompilationIntegrityError};
pub const FEATURE_ID:&str="AFA-benchcompiler-P32-F05";pub const CONTRACT_VERSION:&str="benchcompiler-local_benchmark_compilation_integrity_contract_model/1.0";
pub fn local_benchmark_compilation_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local","contract_model")}
pub fn compile_local_benchmark_compilation_integrity_contract_model(request:&BenchmarkCompileRequest4)->Result<BenchmarkCard7,BenchmarkCompilationIntegrityError>{compile(request,FEATURE_ID,CONTRACT_VERSION,"local","contract_model")}
