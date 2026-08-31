"""Worldgen P29 local scale frontier contract model surface."""
from .worldgen_scale_frontier_support import *
FEATURE_ID="AFA-worldgen-P29-F05"; CONTRACT_VERSION="worldgen-local-scale-frontier-contract_model/1.0"
def worldgen_local_scale_frontier_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract model")
def evaluate_worldgen_local_scale_frontier_contract(request): return evaluate(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract model")

