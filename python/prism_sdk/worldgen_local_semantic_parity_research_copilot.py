"""Worldgen P28 local semantic parity research copilot surface."""
from .worldgen_semantic_parity_support import *
FEATURE_ID="AFA-worldgen-P28-F09"; CONTRACT_VERSION="worldgen-local-semantic-parity-research_copilot/1.0"
def worldgen_local_semantic_parity_research_copilot_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research copilot")
def compare_worldgen_local_semantic_parity_copilot(request): return compare(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="research copilot")

