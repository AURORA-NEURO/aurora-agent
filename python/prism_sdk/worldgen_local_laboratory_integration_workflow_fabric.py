from .worldgen_laboratory_integration_workflow_support import InstrumentWorkflowRequest, InstrumentWorkflowReceipt, schedule, manifest
FEATURE_ID="AFA-worldgen-P11-F13"; CONTRACT_VERSION="worldgen-local-laboratory_integration-workflow/1.0"
def worldgen_local_laboratory_integration_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="InstrumentWorkflowRequest1@1",scale="local single-study",autonomy_tier="A0")
def schedule_worldgen_local_laboratory_integration_workflow(request:InstrumentWorkflowRequest)->InstrumentWorkflowReceipt: return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",require_approval=True,require_federation=False)
