from .worldgen_quality_workflow_support import QualityWorkflowRequest, QualityWorkflowReceipt, schedule, manifest
FEATURE_ID="AFA-worldgen-P07-F14"; CONTRACT_VERSION="worldgen-multimodal-quality-workflow/1.0"
def worldgen_multimodal_quality_control_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="QualityWorkflowRequest1@1",scale="multimodal multi-study",autonomy_tier="A1")
def schedule_worldgen_multimodal_quality_control_workflow(request:QualityWorkflowRequest)->QualityWorkflowReceipt: return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",require_approval=false,require_federation=false)
