"""Worldgen P26 local limitation closure contract model surface."""
from .worldgen_limitation_closure_support import *
FEATURE_ID="AFA-worldgen-P26-F05"; CONTRACT_VERSION="worldgen-local-limitation-closure-contract_model/1.0"
def worldgen_local_limitation_closure_contract_model_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract model")
def close_worldgen_local_limitation_closure_contract(request): return close(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="contract model")

