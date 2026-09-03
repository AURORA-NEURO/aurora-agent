from .worldgen_mechanism_copilot_support import MechanismCopilotRequest, MechanismCopilotReceipt, run, manifest
FEATURE_ID="AFA-worldgen-P08-F12"; CONTRACT_VERSION="worldgen-federated_continual-mechanism-copilot/1.0"
def worldgen_federated_continual_mechanism_exploration_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="MechanismCopilotRequest1@1",scale="federated continual autonomous",autonomy_tier="A1")
def run_worldgen_federated_continual_mechanism_exploration_research_copilot(request:MechanismCopilotRequest)->MechanismCopilotReceipt: return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous",require_approval=false,require_federation=true)
