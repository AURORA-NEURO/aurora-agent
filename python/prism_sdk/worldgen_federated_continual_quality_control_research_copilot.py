from .worldgen_quality_copilot_support import QualityCopilotRequest, QualityCopilotReceipt, run, manifest
FEATURE_ID="AFA-worldgen-P07-F12"; CONTRACT_VERSION="worldgen-federated_continual-quality-copilot/1.0"
def worldgen_federated_continual_quality_control_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="QualityCopilotRequest1@1",scale="federated continual autonomous",autonomy_tier="A1")
def run_worldgen_federated_continual_quality_control_research_copilot(request:QualityCopilotRequest)->QualityCopilotReceipt: return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="federated continual autonomous",require_approval=false,require_federation=true)
