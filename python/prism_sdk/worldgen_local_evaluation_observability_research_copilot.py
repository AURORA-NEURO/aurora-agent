"""Worldgen P23 local evaluation/observability research copilot surface."""
from .worldgen_evaluation_observability_support import *
FEATURE_ID="AFA-worldgen-P23-F09"; CONTRACT_VERSION="worldgen-local-evaluation-observability-copilot/1.0"
def worldgen_local_evaluation_observability_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research copilot")
def run_worldgen_local_evaluation_observability_research_copilot(request): return evaluate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research copilot")

