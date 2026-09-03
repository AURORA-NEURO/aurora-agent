"""Worldgen P32 local single-study contract model surface (F05)."""
from .worldgen_bounded_evolution_support import *
FEATURE_ID="AFA-worldgen-P32-F05"; CONTRACT_VERSION="worldgen-local-bounded-evolution-contract_model/1.0"
def worldgen_local_bounded_evolution_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract model")
def promote_worldgen_local_bounded_evolution_contract(request): return promote(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract model")
