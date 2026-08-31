//! Benchcompiler P32 local research_copilot benchmark-compilation integrity feature.
use super::benchmark_compilation_integrity_support::{manifest,compile,BenchmarkCard7,BenchmarkCompileRequest4,BenchmarkCompilationIntegrityError};
pub const FEATURE_ID:&str="AFA-benchcompiler-P32-F09";pub const CONTRACT_VERSION:&str="benchcompiler-local_benchmark_compilation_integrity_research_copilot/1.0";
pub fn local_benchmark_compilation_integrity_research_copilot_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"local","research_copilot")}
pub fn compile_local_benchmark_compilation_integrity_research_copilot(request:&BenchmarkCompileRequest4)->Result<BenchmarkCard7,BenchmarkCompilationIntegrityError>{compile(request,FEATURE_ID,CONTRACT_VERSION,"local","research_copilot")}
