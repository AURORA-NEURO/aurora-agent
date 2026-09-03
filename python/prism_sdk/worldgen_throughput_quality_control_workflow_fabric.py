from .worldgen_quality_workflow_support import QualityWorkflowRequest, QualityWorkflowReceipt, schedule, manifest
FEATURE_ID="AFA-worldgen-P07-F15"; CONTRACT_VERSION="worldgen-throughput-quality-workflow/1.0"
def worldgen_throughput_quality_control_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="QualityWorkflowRequest1@1",scale="prospective high-throughput",autonomy_tier="A1")
def schedule_worldgen_throughput_quality_control_workflow(request:QualityWorkflowRequest)->QualityWorkflowReceipt: return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",require_approval=false,require_federation=false)
