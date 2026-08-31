"""Benchcompiler P32 local workflow_fabric benchmark-compilation integrity feature."""
from .benchmark_compilation_integrity_support import BenchmarkCard7,BenchmarkCompileRequest4,BenchmarkCompilationIntegrityError,manifest,compile
FEATURE_ID="AFA-benchcompiler-P32-F13";CONTRACT_VERSION="benchcompiler-local_benchmark_compilation_integrity_workflow_fabric/1.0"
def local_benchmark_compilation_integrity_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="workflow_fabric")
def compile_local_benchmark_compilation_integrity_workflow_fabric(request:BenchmarkCompileRequest4)->BenchmarkCard7:return compile(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local",mode="workflow_fabric")
__all__=["FEATURE_ID","CONTRACT_VERSION","local_benchmark_compilation_integrity_workflow_fabric_manifest","compile_local_benchmark_compilation_integrity_workflow_fabric"]
