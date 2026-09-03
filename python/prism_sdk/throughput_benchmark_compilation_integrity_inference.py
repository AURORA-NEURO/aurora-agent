"""Benchcompiler P32 throughput inference benchmark-compilation integrity feature."""
from .benchmark_compilation_integrity_support import BenchmarkCard7,BenchmarkCompileRequest4,BenchmarkCompilationIntegrityError,manifest,compile
FEATURE_ID="AFA-benchcompiler-P32-F03";CONTRACT_VERSION="benchcompiler-throughput_benchmark_compilation_integrity_inference/1.0"
def throughput_benchmark_compilation_integrity_inference_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="throughput",mode="inference")
def compile_throughput_benchmark_compilation_integrity_inference(request:BenchmarkCompileRequest4)->BenchmarkCard7:return compile(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="throughput",mode="inference")
__all__=["FEATURE_ID","CONTRACT_VERSION","throughput_benchmark_compilation_integrity_inference_manifest","compile_throughput_benchmark_compilation_integrity_inference"]
