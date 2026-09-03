"""Worldgen P23 multimodal evaluation/observability research copilot surface."""
from .worldgen_evaluation_observability_support import *
FEATURE_ID="AFA-worldgen-P23-F10"; CONTRACT_VERSION="worldgen-multimodal-evaluation-observability-copilot/1.0"
def worldgen_multimodal_evaluation_observability_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="research copilot")
def run_worldgen_multimodal_evaluation_observability_research_copilot(request): return evaluate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="research copilot")

