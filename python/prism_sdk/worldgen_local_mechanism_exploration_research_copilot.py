from .worldgen_mechanism_copilot_support import MechanismCopilotRequest, MechanismCopilotReceipt, run, manifest
FEATURE_ID="AFA-worldgen-P08-F09"; CONTRACT_VERSION="worldgen-local-mechanism-copilot/1.0"
def worldgen_local_mechanism_exploration_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="MechanismCopilotRequest1@1",scale="local single-study",autonomy_tier="A0")
def run_worldgen_local_mechanism_exploration_research_copilot(request:MechanismCopilotRequest)->MechanismCopilotReceipt: return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",require_approval=true,require_federation=false)
