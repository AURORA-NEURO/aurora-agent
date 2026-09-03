from .worldgen_quality_copilot_support import QualityCopilotRequest, QualityCopilotReceipt, run, manifest
FEATURE_ID="AFA-worldgen-P07-F09"; CONTRACT_VERSION="worldgen-local-quality-copilot/1.0"
def worldgen_local_quality_control_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="QualityCopilotRequest1@1",scale="local single-study",autonomy_tier="A0")
def run_worldgen_local_quality_control_research_copilot(request:QualityCopilotRequest)->QualityCopilotReceipt: return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",require_approval=true,require_federation=false)
