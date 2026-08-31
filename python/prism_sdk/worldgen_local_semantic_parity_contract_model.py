"""Worldgen P28 local semantic parity contract model surface."""
from .worldgen_semantic_parity_support import *
FEATURE_ID="AFA-worldgen-P28-F05"; CONTRACT_VERSION="worldgen-local-semantic-parity-contract_model/1.0"
def worldgen_local_semantic_parity_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract model")
def compare_worldgen_local_semantic_parity_contract(request): return compare(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract model")

