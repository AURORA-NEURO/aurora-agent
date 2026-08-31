"""Local context compilation (AFA-worldgen-P03-F01)."""
from .worldgen_context_compilation_support import *
FEATURE_ID="AFA-worldgen-P03-F01"; CONTRACT_VERSION="worldgen-local-research-context/1.0"; INPUT_SCHEMA="ContextCompilationQuestion1@1"
def worldgen_local_research_context_compilation_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema=INPUT_SCHEMA,scale="local single-study",autonomy_tier="A1")
def compile_worldgen_local_research_context(request):return compile(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,require_federation=False)
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","ContextFact","ContextCompilationRequest","ContextCompilationReceipt","worldgen_local_research_context_compilation_manifest","compile_worldgen_local_research_context"]
