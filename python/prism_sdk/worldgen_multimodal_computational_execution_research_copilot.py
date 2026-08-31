from .worldgen_computational_execution_copilot_support import ExecutionCopilotRequest, ExecutionCopilotReceipt, run, manifest
FEATURE_ID="AFA-worldgen-P12-F10"; CONTRACT_VERSION="worldgen-multimodal-computational_execution-copilot/1.0"
def worldgen_multimodal_computational_execution_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="ExecutionCopilotRequest1@1",scale="multimodal multi-study",autonomy_tier="A1")
def run_worldgen_multimodal_computational_execution_research_copilot(request:ExecutionCopilotRequest)->ExecutionCopilotReceipt: return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",require_approval=False,require_federation=False)
