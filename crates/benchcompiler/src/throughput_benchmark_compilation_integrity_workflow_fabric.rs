//! Benchcompiler P32 throughput workflow_fabric benchmark-compilation integrity feature.
use super::benchmark_compilation_integrity_support::{manifest,compile,BenchmarkCard7,BenchmarkCompileRequest4,BenchmarkCompilationIntegrityError};
pub const FEATURE_ID:&str="AFA-benchcompiler-P32-F15";pub const CONTRACT_VERSION:&str="benchcompiler-throughput_benchmark_compilation_integrity_workflow_fabric/1.0";
pub fn throughput_benchmark_compilation_integrity_workflow_fabric_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"throughput","workflow_fabric")}
pub fn compile_throughput_benchmark_compilation_integrity_workflow_fabric(request:&BenchmarkCompileRequest4)->Result<BenchmarkCard7,BenchmarkCompilationIntegrityError>{compile(request,FEATURE_ID,CONTRACT_VERSION,"throughput","workflow_fabric")}
