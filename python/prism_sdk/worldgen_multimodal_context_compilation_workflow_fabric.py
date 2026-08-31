"""Multimodal context-compilation workflow fabric (AFA-worldgen-P03-F14)."""
from .worldgen_context_workflow_support import *
FEATURE_ID="AFA-worldgen-P03-F14";CONTRACT_VERSION="worldgen-multimodal-context-compilation-workflow/1.0";INPUT_SCHEMA="ContextCompilationQuestion2@1";OUTPUT_SCHEMA="CompiledResearchContext6@1";SCALE="multimodal multi-study"
def worldgen_multimodal_context_compilation_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema=INPUT_SCHEMA,scale=SCALE,autonomy_tier="A1")
def schedule_worldgen_multimodal_context_compilation_workflow(request):return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,require_approval=True,require_federation=False)
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","SCALE","ContextWorkflowRequest","ContextWorkflowReceipt","worldgen_multimodal_context_compilation_workflow_fabric_manifest","schedule_worldgen_multimodal_context_compilation_workflow"]
