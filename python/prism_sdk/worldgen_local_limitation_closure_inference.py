"""Worldgen P26 local limitation closure inference surface."""
from .worldgen_limitation_closure_support import *
FEATURE_ID="AFA-worldgen-P26-F01"; CONTRACT_VERSION="worldgen-local-limitation-closure-inference/1.0"
def worldgen_local_limitation_closure_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")
def close_worldgen_local_limitation_closure(request): return close(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")

