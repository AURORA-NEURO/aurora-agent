from .worldgen_computational_execution_workflow_support import ExecutionWorkflowRequest, ExecutionWorkflowReceipt, schedule, manifest
FEATURE_ID="AFA-worldgen-P12-F15"; CONTRACT_VERSION="worldgen-throughput-computational_execution-workflow/1.0"
def worldgen_throughput_computational_execution_workflow_fabric_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ExecutionWorkflowRequest1@1",scale="prospective high-throughput",autonomy_tier="A1")
def schedule_worldgen_throughput_computational_execution_workflow(request:ExecutionWorkflowRequest)->ExecutionWorkflowReceipt: return schedule(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",require_approval=False,require_federation=False)
