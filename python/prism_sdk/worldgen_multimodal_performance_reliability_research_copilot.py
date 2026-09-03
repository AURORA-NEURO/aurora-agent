"""Worldgen P21 multimodal performance/reliability research-copilot surface."""
from .worldgen_performance_reliability_copilot_support import *
FEATURE_ID="AFA-worldgen-P21-F10"; CONTRACT_VERSION="worldgen-multimodal-performance-reliability-copilot/1.0"
def worldgen_multimodal_performance_reliability_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study")
def run_worldgen_multimodal_performance_reliability_research_copilot(request): return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study")
