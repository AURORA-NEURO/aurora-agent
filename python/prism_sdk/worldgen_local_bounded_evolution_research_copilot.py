"""Worldgen P32 local single-study research copilot surface (F09)."""
from .worldgen_bounded_evolution_support import *
FEATURE_ID="AFA-worldgen-P32-F09"; CONTRACT_VERSION="worldgen-local-bounded-evolution-research_copilot/1.0"
def worldgen_local_bounded_evolution_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research copilot")
def promote_worldgen_local_bounded_evolution_copilot(request): return promote(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research copilot")
