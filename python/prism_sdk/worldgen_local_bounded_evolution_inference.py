"""Worldgen P32 local single-study inference surface (F01)."""
from .worldgen_bounded_evolution_support import *
FEATURE_ID="AFA-worldgen-P32-F01"; CONTRACT_VERSION="worldgen-local-bounded-evolution-inference/1.0"
def worldgen_local_bounded_evolution_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")
def promote_worldgen_local_bounded_evolution(request): return promote(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")
