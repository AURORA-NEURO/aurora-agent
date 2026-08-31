"""Federated continual context compilation (AFA-worldgen-P03-F04)."""
from .worldgen_context_compilation_support import *
FEATURE_ID="AFA-worldgen-P03-F04"; CONTRACT_VERSION="worldgen-federated-continual-research-context/1.0"; INPUT_SCHEMA="ContextCompilationQuestion4@1"
def worldgen_federated_continual_research_context_compilation_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema=INPUT_SCHEMA,scale="federated continual/autonomous",autonomy_tier="A2")
def compile_worldgen_federated_continual_research_context(request):return compile(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,require_federation=True)
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","ContextFact","ContextCompilationRequest","ContextCompilationReceipt","worldgen_federated_continual_research_context_compilation_manifest","compile_worldgen_federated_continual_research_context"]
