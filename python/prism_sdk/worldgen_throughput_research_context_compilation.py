"""Prospective high-throughput context compilation (AFA-worldgen-P03-F03)."""
from .worldgen_context_compilation_support import *
FEATURE_ID="AFA-worldgen-P03-F03"; CONTRACT_VERSION="worldgen-throughput-research-context/1.0"; INPUT_SCHEMA="ContextCompilationQuestion3@1"
def worldgen_throughput_research_context_compilation_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema=INPUT_SCHEMA,scale="prospective high-throughput",autonomy_tier="A2")
def compile_worldgen_throughput_research_context(request):return compile(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,require_federation=True)
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","ContextFact","ContextCompilationRequest","ContextCompilationReceipt","worldgen_throughput_research_context_compilation_manifest","compile_worldgen_throughput_research_context"]
