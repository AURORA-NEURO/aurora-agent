from .worldgen_quality_copilot_support import QualityCopilotRequest, QualityCopilotReceipt, run, manifest
FEATURE_ID="AFA-worldgen-P07-F10"; CONTRACT_VERSION="worldgen-multimodal-quality-copilot/1.0"
def worldgen_multimodal_quality_control_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="QualityCopilotRequest1@1",scale="multimodal multi-study",autonomy_tier="A1")
def run_worldgen_multimodal_quality_control_research_copilot(request:QualityCopilotRequest)->QualityCopilotReceipt: return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",require_approval=false,require_federation=false)
