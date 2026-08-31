"""Worldgen P28 multimodal semantic parity inference surface."""
from .worldgen_semantic_parity_support import *
FEATURE_ID="AFA-worldgen-P28-F02"; CONTRACT_VERSION="worldgen-multimodal-semantic-parity-inference/1.0"
def worldgen_multimodal_semantic_parity_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="inference")
def compare_worldgen_multimodal_semantic_parity(request): return compare(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="inference")

