"""Multimodal context-compilation copilot (AFA-worldgen-P03-F10)."""
from .worldgen_context_copilot_support import *
FEATURE_ID="AFA-worldgen-P03-F10";CONTRACT_VERSION="worldgen-multimodal-context-compilation-copilot/1.0";INPUT_SCHEMA="ContextCompilationQuestion2@1"
def worldgen_multimodal_context_compilation_copilot_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema=INPUT_SCHEMA,scale="multimodal multi-study",autonomy_tier="A1")
def run_worldgen_multimodal_context_compilation_copilot(request):return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,require_approval=True,require_federation=False)
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","ContextCopilotRequest","ContextCopilotReceipt","worldgen_multimodal_context_compilation_copilot_manifest","run_worldgen_multimodal_context_compilation_copilot"]
