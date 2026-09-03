"""Worldgen P26 multimodal limitation closure contract model surface."""
from .worldgen_limitation_closure_support import *
FEATURE_ID="AFA-worldgen-P26-F06"; CONTRACT_VERSION="worldgen-multimodal-limitation-closure-contract_model/1.0"
def worldgen_multimodal_limitation_closure_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="contract model")
def close_worldgen_multimodal_limitation_closure_contract(request): return close(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="contract model")

