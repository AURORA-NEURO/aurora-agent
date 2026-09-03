"""Worldgen P29 local scale frontier research copilot surface."""
from .worldgen_scale_frontier_support import *
FEATURE_ID="AFA-worldgen-P29-F09"; CONTRACT_VERSION="worldgen-local-scale-frontier-research_copilot/1.0"
def worldgen_local_scale_frontier_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research copilot")
def evaluate_worldgen_local_scale_frontier_copilot(request): return evaluate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research copilot")

