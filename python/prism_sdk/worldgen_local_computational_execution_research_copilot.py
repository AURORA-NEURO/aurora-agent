from .worldgen_computational_execution_copilot_support import ExecutionCopilotRequest, ExecutionCopilotReceipt, run, manifest
FEATURE_ID="AFA-worldgen-P12-F09"; CONTRACT_VERSION="worldgen-local-computational_execution-copilot/1.0"
def worldgen_local_computational_execution_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ExecutionCopilotRequest1@1",scale="local single-study",autonomy_tier="A0")
def run_worldgen_local_computational_execution_research_copilot(request:ExecutionCopilotRequest)->ExecutionCopilotReceipt: return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",require_approval=True,require_federation=False)
