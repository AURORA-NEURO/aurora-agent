from .worldgen_quality_copilot_support import QualityCopilotRequest, QualityCopilotReceipt, run, manifest
FEATURE_ID="AFA-worldgen-P07-F11"; CONTRACT_VERSION="worldgen-throughput-quality-copilot/1.0"
def worldgen_throughput_quality_control_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,input_schema="QualityCopilotRequest1@1",scale="prospective high-throughput",autonomy_tier="A1")
def run_worldgen_throughput_quality_control_research_copilot(request:QualityCopilotRequest)->QualityCopilotReceipt: return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="prospective high-throughput",require_approval=false,require_federation=false)
