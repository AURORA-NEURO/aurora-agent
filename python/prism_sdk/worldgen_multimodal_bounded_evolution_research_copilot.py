"""Worldgen P32 multimodal multi-study research copilot surface (F10)."""
from .worldgen_bounded_evolution_support import *
FEATURE_ID="AFA-worldgen-P32-F10"; CONTRACT_VERSION="worldgen-multimodal-bounded-evolution-research_copilot/1.0"
def worldgen_multimodal_bounded_evolution_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="research copilot")
def promote_worldgen_multimodal_bounded_evolution_copilot(request): return promote(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="research copilot")
