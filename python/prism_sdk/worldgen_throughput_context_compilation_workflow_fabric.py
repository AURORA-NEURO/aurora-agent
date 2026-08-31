"""Prospective high-throughput context-compilation workflow fabric (AFA-worldgen-P03-F15)."""
from .worldgen_context_workflow_support import *
FEATURE_ID="AFA-worldgen-P03-F15";CONTRACT_VERSION="worldgen-throughput-context-compilation-workflow/1.0";INPUT_SCHEMA="ContextCompilationQuestion3@1";OUTPUT_SCHEMA="CompiledResearchContext6@1";SCALE="prospective high-throughput"
def worldgen_throughput_context_compilation_workflow_fabric_manifest():return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema=INPUT_SCHEMA,scale=SCALE,autonomy_tier="A2")
def schedule_worldgen_throughput_context_compilation_workflow(request):return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,require_approval=True,require_federation=True)
__all__=["FEATURE_ID","CONTRACT_VERSION","INPUT_SCHEMA","OUTPUT_SCHEMA","SCALE","ContextWorkflowRequest","ContextWorkflowReceipt","worldgen_throughput_context_compilation_workflow_fabric_manifest","schedule_worldgen_throughput_context_compilation_workflow"]
