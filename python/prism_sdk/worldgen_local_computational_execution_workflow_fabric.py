from .worldgen_computational_execution_workflow_support import ExecutionWorkflowRequest, ExecutionWorkflowReceipt, schedule, manifest
FEATURE_ID="AFA-worldgen-P12-F13"; CONTRACT_VERSION="worldgen-local-computational_execution-workflow/1.0"
def worldgen_local_computational_execution_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ExecutionWorkflowRequest1@1",scale="local single-study",autonomy_tier="A0")
def schedule_worldgen_local_computational_execution_workflow(request:ExecutionWorkflowRequest)->ExecutionWorkflowReceipt: return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",require_approval=True,require_federation=False)
