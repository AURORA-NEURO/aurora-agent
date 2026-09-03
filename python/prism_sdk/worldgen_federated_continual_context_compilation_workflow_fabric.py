"""Federated continual context-compilation workflow fabric (AFA-worldgen-P03-F16)."""
from .worldgen_context_workflow_support import *
FEATURE_ID="AFA-worldgen-P03-F16";CONTRACT_VERSION="worldgen-federated-continual-context-compilation-workflow/1.0";INPUT_SCHEMA="ContextCompilationQuestion4@1";OUTPUT_SCHEMA="CompiledResearchContext6@1";SCALE="federated continual/autonomous"
def worldgen_federated_continual_context_compilation_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema=INPUT_SCHEMA,scale=SCALE,autonomy_tier="A2")
def schedule_worldgen_federated_continual_context_compilation_workflow(request):return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,require_approval=True,require_federation=True)
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","SCALE","ContextWorkflowRequest","ContextWorkflowReceipt","worldgen_federated_continual_context_compilation_workflow_fabric_manifest","schedule_worldgen_federated_continual_context_compilation_workflow"]
