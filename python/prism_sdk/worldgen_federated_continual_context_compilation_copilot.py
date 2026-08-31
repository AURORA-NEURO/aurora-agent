"""Federated continual context-compilation copilot (AFA-worldgen-P03-F12)."""
from .worldgen_context_copilot_support import *
FEATURE_ID="AFA-worldgen-P03-F12";CONTRACT_VERSION="worldgen-federated-continual-context-compilation-copilot/1.0";INPUT_SCHEMA="ContextCompilationQuestion4@1"
def worldgen_federated_continual_context_compilation_copilot_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema=INPUT_SCHEMA,scale="federated continual/autonomous",autonomy_tier="A2")
def run_worldgen_federated_continual_context_compilation_copilot(request):return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,require_approval=True,require_federation=True)
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","ContextCopilotRequest","ContextCopilotReceipt","worldgen_federated_continual_context_compilation_copilot_manifest","run_worldgen_federated_continual_context_compilation_copilot"]
