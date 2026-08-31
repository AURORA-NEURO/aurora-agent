"""Benchcompiler P32 local inference benchmark-compilation integrity feature."""
from .benchmark_compilation_integrity_support import BenchmarkCard7,BenchmarkCompileRequest4,BenchmarkCompilationIntegrityError,manifest,compile
FEATURE_ID="AFA-benchcompiler-P32-F01";CONTRACT_VERSION="benchcompiler-local_benchmark_compilation_integrity_inference/1.0"
def local_benchmark_compilation_integrity_inference_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="inference")
def compile_local_benchmark_compilation_integrity_inference(request:BenchmarkCompileRequest4)->BenchmarkCard7:return compile(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="inference")
__all__=["FEATURE_ID","CONTRACT_VERSION","local_benchmark_compilation_integrity_inference_manifest","compile_local_benchmark_compilation_integrity_inference"]
