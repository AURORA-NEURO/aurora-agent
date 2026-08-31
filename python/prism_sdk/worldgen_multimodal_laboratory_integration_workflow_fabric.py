from .worldgen_laboratory_integration_workflow_support import InstrumentWorkflowRequest, InstrumentWorkflowReceipt, schedule, manifest
FEATURE_ID="AFA-worldgen-P11-F14"; CONTRACT_VERSION="worldgen-multimodal-laboratory_integration-workflow/1.0"
def worldgen_multimodal_laboratory_integration_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="InstrumentWorkflowRequest1@1",scale="multimodal multi-study",autonomy_tier="A1")
def schedule_worldgen_multimodal_laboratory_integration_workflow(request:InstrumentWorkflowRequest)->InstrumentWorkflowReceipt: return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",require_approval=False,require_federation=False)
