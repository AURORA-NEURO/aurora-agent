"""Worldgen P21 local performance/reliability research-copilot surface."""
from .worldgen_performance_reliability_copilot_support import *
FEATURE_ID="AFA-worldgen-P21-F09"; CONTRACT_VERSION="worldgen-local-performance-reliability-copilot/1.0"
def worldgen_local_performance_reliability_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study")
def run_worldgen_local_performance_reliability_research_copilot(request): return run(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study")
