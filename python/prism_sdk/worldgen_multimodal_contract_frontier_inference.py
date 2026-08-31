"""Worldgen P25 multimodal contract-frontier inference surface."""
from .worldgen_contract_frontier_support import *
FEATURE_ID="AFA-worldgen-P25-F02"; CONTRACT_VERSION="worldgen-multimodal-contract-frontier-inference/1.0"
def worldgen_multimodal_contract_frontier_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="inference")
def admit_worldgen_multimodal_contract_frontier(request): return admit(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="multimodal multi-study",mode="inference")

