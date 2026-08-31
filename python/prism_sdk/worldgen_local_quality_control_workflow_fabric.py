from .worldgen_quality_workflow_support import QualityWorkflowRequest, QualityWorkflowReceipt, schedule, manifest
FEATURE_ID="AFA-worldgen-P07-F13"; CONTRACT_VERSION="worldgen-local-quality-workflow/1.0"
def worldgen_local_quality_control_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="QualityWorkflowRequest1@1",scale="local single-study",autonomy_tier="A0")
def schedule_worldgen_local_quality_control_workflow(request:QualityWorkflowRequest)->QualityWorkflowReceipt: return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",require_approval=true,require_federation=false)
