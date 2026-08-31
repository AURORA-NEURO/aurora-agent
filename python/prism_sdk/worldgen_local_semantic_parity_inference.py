"""Worldgen P28 local semantic parity inference surface."""
from .worldgen_semantic_parity_support import *
FEATURE_ID="AFA-worldgen-P28-F01"; CONTRACT_VERSION="worldgen-local-semantic-parity-inference/1.0"
def worldgen_local_semantic_parity_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")
def compare_worldgen_local_semantic_parity(request): return compare(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")

