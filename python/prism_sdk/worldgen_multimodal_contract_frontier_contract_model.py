"""Worldgen P25 multimodal contract-frontier contract model surface."""
from .worldgen_contract_frontier_support import *
FEATURE_ID="AFA-worldgen-P25-F06"; CONTRACT_VERSION="worldgen-multimodal-contract-frontier-contract_model/1.0"
def worldgen_multimodal_contract_frontier_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="contract model")
def admit_worldgen_multimodal_contract_frontier_contract(request): return admit(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="contract model")

