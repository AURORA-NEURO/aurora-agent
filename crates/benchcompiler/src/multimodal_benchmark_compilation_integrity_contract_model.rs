//! Benchcompiler P32 multimodal contract_model benchmark-compilation integrity feature.
use super::benchmark_compilation_integrity_support::{manifest,compile,BenchmarkCard7,BenchmarkCompileRequest4,BenchmarkCompilationIntegrityError};
pub const FEATURE_ID:&str="AFA-benchcompiler-P32-F06";pub const CONTRACT_VERSION:&str="benchcompiler-multimodal_benchmark_compilation_integrity_contract_model/1.0";
pub fn multimodal_benchmark_compilation_integrity_contract_model_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal","contract_model")}
pub fn compile_multimodal_benchmark_compilation_integrity_contract_model(request:&BenchmarkCompileRequest4)->Result<BenchmarkCard7,BenchmarkCompilationIntegrityError>{compile(request,FEATURE_ID,CONTRACT_VERSION,"multimodal","contract_model")}
