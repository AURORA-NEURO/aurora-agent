"""Local context-compilation workflow fabric (AFA-worldgen-P03-F13)."""
from .worldgen_context_workflow_support import *
FEATURE_ID="AFA-worldgen-P03-F13";CONTRACT_VERSION="worldgen-local-context-compilation-workflow/1.0";INPUT_SCHEMA="ContextCompilationQuestion1@1";OUTPUT_SCHEMA="CompiledResearchContext6@1";SCALE="local single-study"
def worldgen_local_context_compilation_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema=INPUT_SCHEMA,scale=SCALE,autonomy_tier="A1")
def schedule_worldgen_local_context_compilation_workflow(request):return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,require_approval=False,require_federation=False)
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","SCALE","ContextWorkflowRequest","ContextWorkflowReceipt","worldgen_local_context_compilation_workflow_fabric_manifest","schedule_worldgen_local_context_compilation_workflow"]
