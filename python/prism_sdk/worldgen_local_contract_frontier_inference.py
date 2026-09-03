"""Worldgen P25 local contract-frontier inference surface."""
from .worldgen_contract_frontier_support import *
FEATURE_ID="AFA-worldgen-P25-F01"; CONTRACT_VERSION="worldgen-local-contract-frontier-inference/1.0"
def worldgen_local_contract_frontier_inference_manifest(): return manifest(feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")
def admit_worldgen_local_contract_frontier(request): return admit(request,feature_id=FEATURE_ID,contract_version=CONTRACT_VERSION,scale="local single-study",mode="inference")

