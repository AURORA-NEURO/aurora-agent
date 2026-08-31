//! Benchcompiler P32 multimodal inference benchmark-compilation integrity feature.
use super::benchmark_compilation_integrity_support::{manifest,compile,BenchmarkCard7,BenchmarkCompileRequest4,BenchmarkCompilationIntegrityError};
pub const FEATURE_ID:&str="AFA-benchcompiler-P32-F02";pub const CONTRACT_VERSION:&str="benchcompiler-multimodal_benchmark_compilation_integrity_inference/1.0";
pub fn multimodal_benchmark_compilation_integrity_inference_manifest()->serde_json::Value{manifest(FEATURE_ID,CONTRACT_VERSION,"multimodal","inference")}
pub fn compile_multimodal_benchmark_compilation_integrity_inference(request:&BenchmarkCompileRequest4)->Result<BenchmarkCard7,BenchmarkCompilationIntegrityError>{compile(request,FEATURE_ID,CONTRACT_VERSION,"multimodal","inference")}
