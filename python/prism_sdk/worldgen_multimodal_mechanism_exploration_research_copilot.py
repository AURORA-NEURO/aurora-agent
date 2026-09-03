from .worldgen_mechanism_copilot_support import MechanismCopilotRequest, MechanismCopilotReceipt, run, manifest
FEATURE_ID="AFA-worldgen-P08-F10"; CONTRACT_VERSION="worldgen-multimodal-mechanism-copilot/1.0"
def worldgen_multimodal_mechanism_exploration_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="MechanismCopilotRequest1@1",scale="multimodal multi-study",autonomy_tier="A1")
def run_worldgen_multimodal_mechanism_exploration_research_copilot(request:MechanismCopilotRequest)->MechanismCopilotReceipt: return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",require_approval=false,require_federation=false)
